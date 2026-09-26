//! What dependents compare a query's value by: a 128-bit fingerprint of its `Hash`, two fixed-seed
//! foldhash passes made in one traversal. Fast on the small values most queries give, and the same
//! in every process of one toolchain build, which is what persisted memos are keyed by.

use std::hash::{BuildHasher, Hash, Hasher};

use foldhash::quality::{FixedState, FoldHasher};
use vun_codec::{Decode, Encode};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Fingerprint(u128);

/// The seeds of the two halves.
const SEEDS: [u64; 2] = [0x243f_6a88_85a3_08d3, 0x1319_8a2e_0370_7344];

impl Fingerprint {
    #[must_use]
    pub fn of<T: Hash + ?Sized>(value: &T) -> Self {
        let mut halves = Halves(SEEDS.map(|seed| FixedState::with_seed(seed).build_hasher()));
        value.hash(&mut halves);
        let [lo, hi] = halves.0.map(|h| h.finish());
        Self(u128::from(hi) << 64 | u128::from(lo))
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
