//! Handles into the process's tables, written by content once per stream.

use std::hash::Hash;

use crate::{Blob, Decode, Encode, Malformed, Reader, Writer};

/// A handle whose meaning is an entry of a table of the process, such as a file or an origin: in
/// a stream, its first occurrence writes the entry into the pool of its type and every occurrence
/// is the entry's number; reading it records the entry in the process's table, once per stream.
/// The table is the process's, shared by every database in it, as interned values are: values
/// holding handles are interned, and must mean the same in each.
pub trait Pooled: Copy + Eq + Hash + Send + Sync + 'static {
    /// Names the pool in the stream.
    const POOL: &'static str;

    /// Writes the entry `self` stands for.
    fn write(self, w: &mut Writer<'_>);

    /// Reads an entry and records it in the process's table.
    ///
    /// # Errors
    /// Returns why the bytes are not an entry.
    fn read(r: &mut Reader<'_>) -> Result<Self, Malformed>;
}

/// Implements [`Encode`] and [`Decode`] for a [`Pooled`] handle, as its number in its pool.
#[macro_export]
macro_rules! pooled {
    ($ty:ty) => {
        impl $crate::Encode for $ty {
            fn encode(&self, w: &mut $crate::Writer<'_>) {
                w.pooled(*self);
            }
        }

        impl $crate::Decode for $ty {
            fn decode(r: &mut $crate::Reader<'_>) -> Result<Self, $crate::Malformed> {
                r.pooled()
            }
        }
    };
}

/// The pools of a stream: each pool's entries, in the order they were first written. Written with
/// the entries' bytes, and read with them as bytes or, in place, as [`Blob`]s.
#[derive(Clone, Debug, Default, PartialEq, Eq, Encode, Decode)]
pub struct Pools<B = Vec<u8>>(pub Vec<Pool<B>>);

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Pool<B = Vec<u8>> {
    pub name: String,
    pub entries: Vec<B>,
}

impl Pools {
    /// An input holding every entry, one after another, and the pools as places in it.
    #[must_use]
    pub fn placed(self) -> (Box<[u8]>, Pools<Blob>) {
        let mut input = Vec::new();
        let pools = self.0.into_iter().map(|pool| {
            let entries = pool.entries.iter().map(|entry| {
                let at = input.len();
                input.extend_from_slice(entry);
                Blob { at, len: entry.len() }
            });
            Pool { name: pool.name, entries: entries.collect() }
        });
        let pools = Pools(pools.collect());
        (input.into_boxed_slice(), pools)
    }
}
