//! What names a source file the same way in every process (decisions 59 and 247): its package
//! and its path inside it.

use std::fmt;
use std::str::FromStr;

use vun_codec::{Decode, Encode};
use vun_hash::{Hash, Snapshot};
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

/// Why a text is not a version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotVersion {
    /// The part at this position is not a number without leading zeros.
    Part(usize),
    /// The text has fewer than three parts.
    Short,
    /// The text has more than three parts.
    Long,
}

impl fmt::Display for NotVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Part(at) => {
                write!(f, "part {} of the version is not a number without leading zeros", at + 1)
            }
            Self::Short => f.write_str("the version has fewer than three parts"),
            Self::Long => f.write_str("the version has more than three parts"),
        }
    }
}

impl std::error::Error for NotVersion {}

/// Reads `major.minor.patch`, each part without leading zeros.
impl FromStr for Version {
    type Err = NotVersion;

    fn from_str(text: &str) -> Result<Self, NotVersion> {
        let mut parts = text.split(SEPARATOR).enumerate().map(|(at, p)| {
            let canonical = p == "0" || !p.starts_with('0');
            p.parse()
                .ok()
                .filter(|_| canonical && p.bytes().all(|b| b.is_ascii_digit()))
                .ok_or(NotVersion::Part(at))
        });
        let mut next = || parts.next().unwrap_or(Err(NotVersion::Short));
        let version = Self { major: next()?, minor: next()?, patch: next()? };
        match parts.next() {
            None => Ok(version),
            Some(Ok(_)) => Err(NotVersion::Long),
            Some(Err(err)) => Err(err),
        }
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

    /// The file at `relative`, a path from this file's directory with components joined by
    /// [`Snapshot::PATH_SEPARATOR`], in the same package; `None` for an absolute path or one
    /// leading out of the package.
    #[must_use]
    pub fn beside(&self, relative: &str) -> Option<Self> {
        const HERE: &str = ".";
        const UP: &str = "..";
        let separator = Snapshot::PATH_SEPARATOR;
        if relative.starts_with(separator) {
            return None;
        }
        let mut parts: Vec<&str> = self.path.as_str().split(separator).collect();
        parts.pop();
        for part in relative.split(separator) {
            match part {
                "" | HERE => {}
                UP => {
                    parts.pop()?;
                }
                part => parts.push(part),
            }
        }
        Some(Self { package: self.package, path: Symbol::intern(&parts.join(&separator.to_string())) })
    }
}
