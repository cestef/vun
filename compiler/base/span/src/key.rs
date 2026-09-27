//! What names a source file the same way in every process (decisions 59 and 247): its package
//! and its path inside it.

use std::fmt;

use vun_codec::{Decode, Encode};
use vun_hash::Hash;
use vun_intern::Symbol;

/// A package version, `major.minor.patch`, compared numerically.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

/// Separates the parts of a version.
const SEPARATOR: char = '.';

impl Version {
    /// Parses `major.minor.patch`, each part without leading zeros.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = text.split(SEPARATOR).map(|p| {
            let canonical = p == "0" || !p.starts_with('0');
            p.parse().ok().filter(|_| canonical && p.bytes().all(|b| b.is_ascii_digit()))
        });
        let version = Self { major: parts.next()??, minor: parts.next()??, patch: parts.next()?? };
        parts.next().is_none().then_some(version)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{SEPARATOR}{}{SEPARATOR}{}", self.major, self.minor, self.patch)
    }
}

/// A package as its files are named.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Encode, Decode)]
pub enum PackageId {
    /// One in the store, which never changes: by the hash of its contents, so that a fork of a
    /// release is a package of its own.
    Stored(Hash),
    /// One on disk, edited in place: by the name, identity and version it declares, which its
    /// edits keep.
    Local { name: Symbol, id: u64, version: Version },
    /// The files outside any package.
    Loose,
}

/// A file of a package, by its path inside the package's directory, components joined by `/`; a
/// loose file by its whole path.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Encode, Decode)]
pub struct FileRef {
    pub package: PackageId,
    pub path: Symbol,
}

impl FileRef {
    /// A file outside any package, at `path`.
    #[must_use]
    pub fn loose(path: &str) -> Self {
        Self { package: PackageId::Loose, path: Symbol::intern(path) }
    }
}
