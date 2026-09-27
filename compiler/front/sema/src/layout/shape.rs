//! Where the parts of a value are: sizes and alignments, the offset of each part, how an enum
//! tells its variants apart, and the unused bit patterns an enclosing enum can keep its tag in.

use crate::int::Integer;
use vun_codec::{Decode, Encode, Malformed, Reader};

/// Size and alignment in bytes, and where unused patterns are.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Layout {
    pub size: u64,
    pub align: u64,
    /// The largest run of patterns no value uses, if any.
    pub niche: Option<Niche>,
}

/// `count` bit patterns no value uses, from `start` on and wrapping, of the unsigned integer of
/// `bytes` bytes at `offset`: past the range of an integer, the null pointer, the tags past an
/// enum's variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode)]
pub struct Niche {
    pub offset: u64,
    pub bytes: u64,
    pub start: u128,
    pub count: u128,
}

/// A niche read holds a pattern.
impl Decode for Niche {
    fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        let (offset, bytes, start, count) = <(u64, u64, u128, u128)>::decode(r)?;
        (count > 0).then_some(Self { offset, bytes, start, count }).ok_or(Malformed::Invariant)
    }
}

/// A type's layout and where its parts are.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Shape {
    pub layout: Layout,
    pub parts: Parts,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Parts {
    /// A scalar, or a value without parts.
    Whole,
    /// The byte offset of each part, in order: a struct's fields in declaration order, a tuple's
    /// items, a record's fields, a closure's captures, a slice borrow's pointer and length.
    Sequence(Vec<u64>),
    /// `len` items, `stride` bytes apart.
    Array { stride: u64, len: u64 },
    /// The bit offset of each field of a `#[layout.bits(uN)]` struct, least significant first.
    Bits(Vec<u64>),
    /// An enum: how its variant is told, and the byte offsets of each variant's payload items.
    Enum { tag: Tag, payloads: Vec<Vec<u64>> },
}

/// How an enum's value tells which variant it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Tag {
    /// One variant, or none: nothing is stored.
    Alone,
    /// The variant's index, an unsigned integer of `bytes` bytes at offset 0.
    Index { bytes: u64 },
    /// The payload-free variant `empty` is the pattern `value` of the unsigned integer of `bytes`
    /// bytes at `offset`, in the other variant's payload; any other pattern is that variant
    /// (the niche rule).
    Niche { empty: usize, offset: u64, bytes: u64, value: u128 },
}

impl Layout {
    pub(crate) const EMPTY: Self = Self { size: 0, align: 1, niche: None };

    /// How many patterns no value uses.
    #[must_use]
    pub fn niches(&self) -> u128 {
        self.niche.map_or(0, |n| n.count)
    }

    /// A scalar of `bytes` bytes, aligned to its size.
    pub(crate) const fn scalar(bytes: u64, niche: Option<Niche>) -> Self {
        Self { size: bytes, align: bytes, niche }
    }

    /// Parts laid out one after the other, each at its alignment, the whole padded to the
    /// largest: a struct or a tuple, and the offset of each part. Its unused patterns are the
    /// largest run of a part's, the first one on a tie.
    pub(crate) fn sequence(parts: impl IntoIterator<Item = Self>) -> (Self, Vec<u64>) {
        Self::placed(parts.into_iter().map(|p| (p, None))).expect("parts placed in order fit")
    }

    /// The parts in order, each at the offset given, or else at the first offset its alignment
    /// allows after the one before; `None` when a given offset overlaps the part before or
    /// misaligns the part, or the size passes `u64`.
    pub(crate) fn placed(parts: impl IntoIterator<Item = (Self, Option<u64>)>) -> Option<(Self, Vec<u64>)> {
        let mut whole = Self::EMPTY;
        let mut offsets = Vec::new();
        for (part, at) in parts {
            let offset = match at {
                Some(at) if at < whole.size || at % part.align != 0 => return None,
                Some(at) => at,
                None => whole.size.checked_next_multiple_of(part.align)?,
            };
            offsets.push(offset);
            whole.size = offset.checked_add(part.size)?;
            whole.align = whole.align.max(part.align);
            if let Some(niche) = part.niche
                && niche.count > whole.niches()
            {
                whole.niche = Some(niche.within(offset));
            }
        }
        whole.size = whole.size.checked_next_multiple_of(whole.align)?;
        Some((whole, offsets))
    }
}

impl Niche {
    /// The patterns of an unsigned integer of `bytes` bytes at offset 0 outside the run of
    /// `used` patterns ending at `last`, taken modulo the integer's width; `None` when every
    /// pattern is used.
    pub(crate) fn past(bytes: u64, last: &Integer, used: &Integer) -> Option<Self> {
        let modulus = Integer::power_of_two(u32::try_from(bytes * u64::from(u8::BITS)).ok()?);
        let count = (&modulus - used).to_u128().filter(|&c| c > 0)?;
        let start = (last + &Integer::from(1)).modulo(&modulus)?.to_u128()?;
        Some(Self { offset: 0, bytes, start, count })
    }

    /// This run, in a value placing its integer `offset` bytes further.
    #[must_use]
    pub fn within(self, offset: u64) -> Self {
        Self { offset: self.offset + offset, ..self }
    }

    /// The first pattern, and the run left after it.
    #[must_use]
    pub fn take(self) -> (u128, Option<Self>) {
        let bits = self.bytes * u64::from(u8::BITS);
        let mask = if bits >= u64::from(u128::BITS) { u128::MAX } else { (1 << bits) - 1 };
        let rest = Self { start: self.start.wrapping_add(1) & mask, count: self.count - 1, ..self };
        (self.start, (rest.count > 0).then_some(rest))
    }
}

impl Shape {
    /// A value without parts.
    pub(crate) fn whole(layout: Layout) -> Self {
        Self { layout, parts: Parts::Whole }
    }

    /// Parts one after the other, as [`Layout::sequence`] places them.
    pub(crate) fn sequence(parts: impl IntoIterator<Item = Layout>) -> Self {
        let (layout, offsets) = Layout::sequence(parts);
        Self { layout, parts: Parts::Sequence(offsets) }
    }
}
