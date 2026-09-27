//! A fast hash that is the same in every process of one toolchain build: what the identities of
//! in-memory entities that persisted data holds (symbols, interned values, origins) hash by, and
//! at 128 bits what query values are compared by (their fingerprints). It is not a content hash:
//! another toolchain build may hash differently, and everything saved by these hashes is named by
//! the toolchain that wrote it.

use std::hash::{BuildHasher, Hash, Hasher};

use foldhash::quality::{FixedState, FoldHasher};

pub struct Stable;

impl Stable {
    const SEED: u64 = 0xa409_3822_299f_31d0;

    #[must_use]
    pub fn hasher() -> FoldHasher<'static> {
        FixedState::with_seed(Self::SEED).build_hasher()
    }

    #[must_use]
    pub fn of<T: Hash + ?Sized>(value: &T) -> u64 {
        let mut hasher = Self::hasher();
        value.hash(&mut hasher);
        hasher.finish()
    }

    /// The seeds of the two halves of a wide hash.
    const WIDE: [u64; 2] = [0x243f_6a88_85a3_08d3, 0x1319_8a2e_0370_7344];

    /// A 128-bit hash of `value`: two passes of differently seeded hashers made in one traversal,
    /// fast on small values, and wide enough that values are compared by it.
    #[must_use]
    pub fn wide<T: Hash + ?Sized>(value: &T) -> u128 {
        let mut halves = Halves(Self::WIDE.map(|seed| FixedState::with_seed(seed).build_hasher()));
        value.hash(&mut halves);
        let [lo, hi] = halves.0.map(|h| h.finish());
        u128::from(hi) << 64 | u128::from(lo)
    }
}

/// Both halves' hashers, fed the same writes.
struct Halves([FoldHasher<'static>; 2]);

/// Integers go through foldhash's own integer paths, faster than their bytes.
macro_rules! forward {
    ($($write:ident: $int:ty),*) => {
        $(fn $write(&mut self, n: $int) {
            self.0.iter_mut().for_each(|h| h.$write(n));
        })*
    };
}

impl Hasher for Halves {
    fn write(&mut self, bytes: &[u8]) {
        self.0.iter_mut().for_each(|h| h.write(bytes));
    }

    forward!(write_u8: u8, write_u16: u16, write_u32: u32, write_u64: u64, write_u128: u128, write_usize: usize);

    fn finish(&self) -> u64 {
        self.0[0].finish()
    }
}
