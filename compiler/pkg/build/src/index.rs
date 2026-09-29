//! Package indexes (spec 12, section 3): servers that list the releases of an identity and serve
//! their archives, in the style of Go's module proxy. An index is a mirror, never an authority:
//! what it serves is verified against the hash its listing states.
//!
//! Under a base URL, `<identity>/list` holds one `version hash` line per release, and
//! `<identity>/<version>.tar.zst` the archive; `<identity>` is written `id/major`.

use vun_fetch::Format;
use vun_hash::Hash;
use vun_uri::Uri;

use crate::manifest::{Identity, Version};

/// Names the listing of an identity.
const LIST: &str = "list";

/// An index, by base URL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Index<'a> {
    pub base: &'a Uri,
}

/// The releases an index lists for one identity, in version order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Listing {
    pub releases: Vec<(Version, Hash)>,
}

impl Index<'_> {
    /// Where the releases of `identity` are listed.
    #[must_use]
    pub fn listing(&self, identity: Identity) -> Uri {
        let [id, major] = identity.parts();
        self.base.with_segments([id.as_str(), &major, LIST])
    }

    /// Where the archive of `version` of `identity` is.
    #[must_use]
    pub fn archive(&self, identity: Identity, version: Version) -> Uri {
        let [id, major] = identity.parts();
        let archive = format!("{version}{}", Format::PUBLISHED.suffix());
        self.base.with_segments([id.as_str(), &major, &archive])
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

    /// The largest release of the major of `current`, if newer than it.
    #[must_use]
    pub fn newer(&self, current: Version) -> Option<(Version, Hash)> {
        let major = self.releases.iter().rev().find(|(v, _)| v.major == current.major);
        major.copied().filter(|&(v, _)| v > current)
    }
}

impl std::fmt::Display for Listing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.releases.iter().try_for_each(|(version, hash)| writeln!(f, "{version} {hash}"))
    }
}

#[cfg(test)]
mod tests;
