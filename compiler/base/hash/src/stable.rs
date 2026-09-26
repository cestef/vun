//! A fast hash that is the same in every process of one toolchain build: what the identities of
//! in-memory entities that persisted data holds (symbols, interned values, origins) hash by. It
//! is not a content hash: another toolchain build may hash differently, and everything saved by
//! these hashes is named by the toolchain that wrote it.

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
}
