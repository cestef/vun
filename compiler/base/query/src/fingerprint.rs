//! What dependents compare a query's value by: a 128-bit fingerprint of its `Hash`
//! ([`Stable::wide`]), fast on the small values most queries give, and the same in every process
//! of one toolchain build, which is what persisted memos are keyed by.

use std::hash::Hash;

use vun_codec::{Decode, Encode, Malformed, Reader, Writer};
use vun_hash::Stable;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fingerprint(u128);

/// Its sixteen bytes, as a hash has no short form.
impl Encode for Fingerprint {
    fn encode(&self, w: &mut Writer<'_>) {
        self.0.to_le_bytes().encode(w);
    }
}

impl Decode for Fingerprint {
    fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        <[u8; 16]>::decode(r).map(|bytes| Self(u128::from_le_bytes(bytes)))
    }
}

impl Fingerprint {
    /// The fingerprint of a set with `self` and `other` in it: added, so that the order members
    /// come in does not matter.
    #[must_use]
    pub(crate) fn with(self, other: Self) -> Self {
        Self(self.0.wrapping_add(other.0))
    }

    /// The fingerprint of a set without `other`, which it has.
    #[must_use]
    pub(crate) fn without(self, other: Self) -> Self {
        Self(self.0.wrapping_sub(other.0))
    }

    #[must_use]
    pub fn of<T: Hash + ?Sized>(value: &T) -> Self {
        Self(Stable::wide(value))
    }
}
