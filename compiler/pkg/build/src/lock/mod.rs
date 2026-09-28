//! The lockfile, `lock.vun` beside the manifest (spec 12, section 4): a Vun literal file
//! recording every version the graph requires, with the hash it was verified against, one entry
//! per line, sorted, so that merges never conflict. The selected version of an identity is the
//! largest recorded. It is read without evaluation and written by the literal writer, as
//! manifests are, and is no source namespace.

mod read;
#[cfg(test)]
mod tests;
mod write;

use vun_hash::Hash;

use crate::manifest::{Identity, Version};

/// The lockfile's name, next to the manifest.
pub const FILE: &str = vun_span::source_file!("lock");

/// The version of the lockfile's layout this toolchain reads and writes.
pub const LAYOUT: u32 = 1;

/// A lockfile: the required packages, sorted by name, version and identity, and the identities
/// the root replaces.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Lock {
    entries: Vec<Locked>,
    replaced: Vec<Replaced>,
}

/// An identity the root manifest replaces, and what with.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Replaced {
    pub name: String,
    pub identity: Identity,
    pub with: Replacing,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Replacing {
    /// A stored package, by tree hash.
    Stored(Hash),
    /// A directory, by path from the root.
    Path(String),
}

/// One required package.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Locked {
    /// The name the first dependent to require it binds it to.
    pub name: String,
    pub version: Version,
    /// `None` for a foreign package, which is identified by its hash alone.
    pub identity: Option<Identity>,
    pub hash: Hash,
    /// The dependent that requires it, when that is not the project.
    pub via: Option<String>,
}

/// The declarations and fields of a lockfile.
mod field {
    pub(super) const LAYOUT: &str = "version";
    pub(super) const DEPS: &str = "deps";
    pub(super) const REPLACE: &str = "replace";

    pub(super) const NAME: &str = "name";
    pub(super) const VERSION: &str = "version";
    pub(super) const ID: &str = "id";
    pub(super) const MAJOR: &str = "major";
    pub(super) const HASH: &str = "hash";
    pub(super) const VIA: &str = "via";
    pub(super) const PATH: &str = "path";
    /// The identity of a foreign package, written `.id = .foreign`.
    pub(super) const FOREIGN: &str = "foreign";

    pub(super) const ENTRY: &[&str] = &[NAME, VERSION, ID, HASH, VIA];
    pub(super) const REPLACEMENT: &[&str] = &[NAME, ID, HASH, PATH];
    pub(super) const IDENTITY: &[&str] = &[ID, MAJOR];
}

impl Lock {
    /// Whether it records nothing: no version selected, nothing replaced.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.replaced.is_empty()
    }

    /// The lock of `entries`, in their canonical order.
    #[must_use]
    pub fn new(mut entries: Vec<Locked>) -> Self {
        entries.sort();
        entries.dedup();
        Self { entries, replaced: Vec::new() }
    }

    /// This lock with `replaced` recorded, in their canonical order.
    #[must_use]
    pub fn replacing(mut self, mut replaced: Vec<Replaced>) -> Self {
        replaced.sort();
        replaced.dedup();
        self.replaced = replaced;
        self
    }

    /// What replaces `identity`, if the root replaces it.
    #[must_use]
    pub fn replacement(&self, identity: Identity) -> Option<&Replaced> {
        self.replaced.iter().find(|r| r.identity == identity)
    }

    #[must_use]
    pub fn entries(&self) -> &[Locked] {
        &self.entries
    }

    /// The entry selected for `identity`: the largest version required.
    #[must_use]
    pub fn selected(&self, identity: Identity) -> Option<&Locked> {
        self.entries.iter().filter(|e| e.identity == Some(identity)).max_by_key(|e| e.version)
    }
}
