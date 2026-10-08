//! A query's saved keys: each led by its hash, and packed so that one is found without reading
//! the others.

use vun_codec::{Blob, Decode, Encode, Malformed, Reader, Writer};

use crate::fingerprint::Fingerprint;

/// What leads a saved key: the stable hash of its value, by which it is found, written as eight
/// bytes so that it is read without the rest, and the fingerprint of its memo if it had one when
/// first saved: the one its memo and reads of it saw unless they record their own.
#[derive(Clone, Copy, Debug, Encode, Decode)]
pub(crate) struct KeyHead {
    pub(super) hash: Fixed,
    pub(super) fingerprint: Option<Fingerprint>,
}

/// A `u64` written as its eight bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Fixed(pub(super) u64);

impl Encode for Fixed {
    fn encode(&self, w: &mut Writer<'_>) {
        w.bytes(&self.0.to_le_bytes());
    }
}

impl Decode for Fixed {
    fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        let bytes: [u8; 8] = r.take(8)?.try_into().map_err(|_| Malformed::Truncated)?;
        Ok(Self(u64::from_le_bytes(bytes)))
    }
}

/// A key, its [`KeyHead`] first.
#[derive(Clone, Encode, Decode)]
pub(crate) struct SavedKey<B = Vec<u8>> {
    pub(super) head: KeyHead,
    pub(super) key: B,
    /// A [`SavedMemo`].
    pub(super) memo: Option<B>,
}

impl<B> SavedKey<B> {
    pub(super) fn hash(&self) -> u64 {
        self.head.hash.0
    }

    pub(super) fn fingerprint(&self) -> Option<Fingerprint> {
        self.head.fingerprint
    }
}

/// A query's keys in the form that finds one without reading the others: where each starts, and
/// a table finding the first of a hash by probing, then the keys, each a [`SavedKey`].
#[derive(Encode, Decode)]
pub(crate) struct PackedKeys<B = Vec<u8>> {
    pub(super) count: u32,
    /// The start of each key in `data`, a little-endian `u32` each.
    pub(super) starts: B,
    /// A power of two of little-endian `u32`s, each the position plus one of the first key of a
    /// hash, or none: probed from the hash, a slot at a time.
    pub(super) table: B,
    pub(super) data: B,
}

impl PackedKeys<Blob> {
    /// The keys in order, read from `input`, the bytes the places are in.
    pub(super) fn read(&self, input: &[u8]) -> Result<Vec<SavedKey<Blob>>, Malformed> {
        let starts = self.starts.of(input);
        (0..self.count as usize)
            .map(|at| {
                let start =
                    starts.get(at * 4..).and_then(|s| s.first_chunk::<4>()).ok_or(Malformed::Truncated)?;
                let start = u32::from_le_bytes(*start) as usize;
                let len = self.data.len.checked_sub(start).ok_or(Malformed::Truncated)?;
                Blob { at: self.data.at + start, len }.front(input)
            })
            .collect()
    }
}

impl PackedKeys {
    #[expect(clippy::cast_possible_truncation, reason = "only the low bits of a hash index the table")]
    pub(super) fn pack(keys: &[SavedKey]) -> Self {
        let (mut starts, mut data) = (Vec::with_capacity(keys.len() * 4), Vec::new());
        for key in keys {
            starts.extend_from_slice(
                &u32::try_from(data.len()).expect("a query's keys fit in 4 GiB").to_le_bytes(),
            );
            data.extend_from_slice(&key.encoded());
        }
        let size = (keys.len() * 2).next_power_of_two().max(2);
        let mut table = vec![0u32; size];
        for (at, key) in (1u32..).zip(keys) {
            let mut slot = key.hash() as usize & (size - 1);
            loop {
                match table[slot] {
                    0 => {
                        table[slot] = at;
                        break;
                    }
                    held if keys[held as usize - 1].hash() == key.hash() => break,
                    _ => slot = (slot + 1) & (size - 1),
                }
            }
        }
        let table = table.iter().flat_map(|slot| slot.to_le_bytes()).collect();
        Self {
            count: u32::try_from(keys.len()).expect("fewer than 2^32 keys per query"),
            starts,
            table,
            data,
        }
    }
}
