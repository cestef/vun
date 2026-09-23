//! The local store: fetched packages by content hash (spec 12, section 3), nested as
//! `<project>/.overt/store/<algorithm>/<first digest characters>/<rest of the digest>/`, so that
//! no directory grows past a few thousand entries and every path is derived from the hash alone.

use std::path::{Path, PathBuf};

use overt_hash::Hash;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    /// A project's local state, ignored by version control.
    pub const LOCAL: &str = ".overt";
    pub const DIR: &str = "store";
    /// Digest characters naming the first level: 32 * 32 = 1024 directories in base32.
    pub const SPLIT: usize = 2;

    /// The store of the project in `dir`.
    #[must_use]
    pub fn in_project(dir: &Path) -> Self {
        Self { root: dir.join(Self::LOCAL).join(Self::DIR) }
    }

    /// Where the package with tree hash `hash` is unpacked.
    #[must_use]
    pub fn path(&self, hash: Hash) -> PathBuf {
        let digest = hash.digest();
        let (first, rest) = digest.split_at(Self::SPLIT);
        self.root.join(hash.algorithm().name()).join(first).join(rest)
    }
}

#[cfg(test)]
mod tests;
