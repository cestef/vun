//! The local store: fetched packages by content hash (spec 12, section 3). An entry is nested as
//! `<project>/.overt/store/<algorithm>/<first digest characters>/<rest of the digest>/`, so that
//! no directory grows past a few thousand entries and every path is derived from the hash alone;
//! it holds the unpacked `package/` and the `snapshot` of its file hashes verified when fetched.
//! A fetch unpacks under `staging/` and moves the entry in place once verified.

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
    pub const PACKAGE: &str = "package";
    pub const SNAPSHOT: &str = "snapshot";
    pub const STAGING: &str = "staging";

    /// The store of the project in `dir`.
    #[must_use]
    pub fn in_project(dir: &Path) -> Self {
        Self { root: dir.join(Self::LOCAL).join(Self::DIR) }
    }

    /// The entry of the package with tree hash `hash`.
    #[must_use]
    pub fn entry(&self, hash: Hash) -> PathBuf {
        let digest = hash.digest();
        let (first, rest) = digest.split_at(Self::SPLIT);
        self.root.join(hash.algorithm().name()).join(first).join(rest)
    }

    /// Where the package is unpacked.
    #[must_use]
    pub fn package(&self, hash: Hash) -> PathBuf {
        self.entry(hash).join(Self::PACKAGE)
    }

    /// The file hashes recorded when the package was fetched.
    #[must_use]
    pub fn snapshot(&self, hash: Hash) -> PathBuf {
        self.entry(hash).join(Self::SNAPSHOT)
    }

    /// Where a fetch of `hash` builds its entry before moving it in place.
    #[must_use]
    pub fn staging(&self, hash: Hash) -> PathBuf {
        self.root.join(Self::STAGING).join(hash.algorithm().name()).join(hash.digest())
    }
}

#[cfg(test)]
mod tests;
