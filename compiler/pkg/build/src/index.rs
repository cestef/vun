//! Package indexes (spec 12, section 3): servers that list the releases of an identity and serve
//! their archives, in the style of Go's module proxy. An index is a mirror, never an authority:
//! what it serves is verified against the hash its listing states.
//!
//! Under a base URL, `<identity>/list` holds one `version hash` line per release, and
//! `<identity>/<version>.tar.zst` the archive; `<identity>` is written `id/major`.

use std::error::Error;
use std::fmt;
use std::str::FromStr;

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
    /// The largest release of the major of `current`, if newer than it.
    #[must_use]
    pub fn newer(&self, current: Version) -> Option<(Version, Hash)> {
        let major = self.releases.iter().rev().find(|(v, _)| v.major == current.major);
        major.copied().filter(|&(v, _)| v > current)
    }
}

/// Where a listing is not `version hash` lines, and why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Misread {
    /// The line, from 1.
    pub line: usize,
    pub reason: Reason,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// The line is not a version and a hash.
    Fields,
    /// The version does not read.
    Version,
    /// The hash does not read.
    Hash,
}

impl FromStr for Listing {
    type Err = Misread;

    fn from_str(text: &str) -> Result<Self, Misread> {
        let mut releases = Vec::new();
        for (number, line) in text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
            let at = |reason| Misread { line: number + 1, reason };
            let (version, hash) = line.split_once(' ').ok_or(at(Reason::Fields))?;
            let version = version.parse::<Version>().map_err(|_| at(Reason::Version))?;
            releases.push((version, hash.trim().parse::<Hash>().map_err(|_| at(Reason::Hash))?));
        }
        releases.sort();
        Ok(Self { releases })
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Fields => "not a version and a hash",
            Self::Version => "the version does not read",
            Self::Hash => "the hash does not read",
        })
    }
}

impl fmt::Display for Misread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.reason)
    }
}

impl Error for Misread {}

impl std::fmt::Display for Listing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.releases.iter().try_for_each(|(version, hash)| writeln!(f, "{version} {hash}"))
    }
}

#[cfg(test)]
mod tests;
