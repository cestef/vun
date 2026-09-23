//! Package indexes (spec 12, section 3): servers that list the releases of an identity and serve
//! their archives, in the style of Go's module proxy. An index is a mirror, never an authority:
//! what it serves is verified against the hash its listing states.
//!
//! Under a base URL, `<identity>/list` holds one `version hash` line per release, and
//! `<identity>/<version>.ovpkg` the archive; `<identity>` is written `id/major`.

use overt_hash::Hash;
use overt_pkg::Archive;

use crate::manifest::{Identity, Version};

/// Names the listing of an identity.
const LIST: &str = "list";
/// Separates the parts of an index URL.
const SEPARATOR: char = '/';

/// An index, by base URL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Index<'a> {
    pub base: &'a str,
}

/// The releases an index lists for one identity, in version order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Listing {
    pub releases: Vec<(Version, Hash)>,
}

impl Index<'_> {
    /// Where the releases of `identity` are listed.
    #[must_use]
    pub fn listing(&self, identity: Identity) -> String {
        format!("{}{SEPARATOR}{identity}{SEPARATOR}{LIST}", self.base.trim_end_matches(SEPARATOR))
    }

    /// Where the archive of `version` of `identity` is.
    #[must_use]
    pub fn archive(&self, identity: Identity, version: Version) -> String {
        let base = self.base.trim_end_matches(SEPARATOR);
        format!("{base}{SEPARATOR}{identity}{SEPARATOR}{version}{}", Archive::SUFFIX)
    }
}

impl Listing {
    /// Reads `version hash` lines; `None` for a line that is neither.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut releases = Vec::new();
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let (version, hash) = line.split_once(' ')?;
            releases.push((Version::parse(version)?, Hash::parse(hash.trim())?));
        }
        releases.sort();
        Some(Self { releases })
    }

    /// The largest release, if newer than `current`.
    #[must_use]
    pub fn newer(&self, current: Version) -> Option<(Version, Hash)> {
        self.releases.last().copied().filter(|&(v, _)| v > current)
    }
}

impl std::fmt::Display for Listing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.releases.iter().try_for_each(|(version, hash)| writeln!(f, "{version} {hash}"))
    }
}

#[cfg(test)]
mod tests;
