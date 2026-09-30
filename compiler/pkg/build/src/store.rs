//! The store: fetched packages by content hash (spec 12, section 3), shared by every project of a
//! user. An entry is nested as `<store>/<algorithm>/<first digest characters>/<rest of the
//! digest>/`, so that no directory grows past a few thousand entries and every path is derived
//! from the hash alone; it holds the unpacked `package/`, the `snapshot` of its file hashes
//! verified when fetched, and a `label` naming it for people. A fetch unpacks under `staging/`, in
//! a directory of its process, and moves the entry in place once verified unless another process
//! already did. Projects register their lockfiles under `roots/`, and
//! collection keeps what some registered lockfile records.

mod collect;
mod staging;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use vun_hash::Hash;

use crate::dirs::Dirs;

pub use collect::{Collected, Listed, Unparsed};
pub use staging::Staging;

/// What a store entry is called for people: the name and version a dependent declared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Label<'a> {
    pub name: &'a str,
    pub version: crate::manifest::Version,
}

impl std::fmt::Display for Label<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.name, self.version)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    /// Names the store's directory instead of the user's cache.
    pub const ENV: &str = "VUN_STORE";
    /// The toolchain's directory in the user's cache.
    pub const CACHE: &str = "vun";
    pub const DIR: &str = "store";
    /// Digest characters naming the first level: 32 * 32 = 1024 directories in base32.
    pub const SPLIT: usize = 2;
    pub const PACKAGE: &str = "package";
    pub const SNAPSHOT: &str = "snapshot";
    pub const LABEL: &str = "label";
    pub const STAGING: &str = "staging";
    pub const ROOTS: &str = "roots";
    /// The libraries' memos shared between projects (spec 13, section 2).
    pub const MEMOS: &str = "memos";

    /// The directory of the libraries' memos shared between projects.
    #[must_use]
    pub fn memos(&self) -> PathBuf {
        self.root.join(Self::MEMOS)
    }

    /// The store in `dir`.
    #[must_use]
    pub fn at(dir: &Path) -> Self {
        Self { root: dir.to_path_buf() }
    }

    /// The user's store: the directory named by [`Self::ENV`], or `vun/store` in the user's
    /// cache directory (XDG on Unix, including macOS, as command-line tools expect; the known
    /// folder on Windows). A user without a home keeps one in the working directory's
    /// [`vun_fs::WORK`], never in a directory other users can write.
    #[must_use]
    pub fn user() -> Self {
        let named = std::env::var_os(Self::ENV).map(PathBuf::from);
        let cache = || {
            Dirs::user().map_or_else(
                || std::env::current_dir().unwrap_or_default().join(vun_fs::WORK).join(Self::DIR),
                |dirs| dirs.cache.join(Self::CACHE).join(Self::DIR),
            )
        };
        Self::at(&named.unwrap_or_else(cache))
    }

    /// Its directory.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.root
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

    /// The name and version a dependent declared the package under, for people reading the store.
    #[must_use]
    pub fn label(&self, hash: Hash) -> PathBuf {
        self.entry(hash).join(Self::LABEL)
    }
}
