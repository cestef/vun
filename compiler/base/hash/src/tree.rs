//! Tree hashes: a Merkle tree over directory entries. A file's node is the hash of its contents; a
//! directory's is the hash of its entries in name order, each a kind, a name and the child's hash.
//! Names are single path components, so a tree hashes the same on every platform, and a file's
//! hash is the one every cache already computes.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Component, Path};

use overt_fs::Tree;

use super::{Algorithm, Hash};

/// Separates directory nodes from file contents, so that no file hashes like a directory.
const DIRECTORY_CONTEXT: &str = "overt 2026 directory node";

/// The hash of every file of a tree, by its path, from which the tree's hash is computed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    files: BTreeMap<Vec<String>, Hash>,
}

/// How a file differs between two snapshots.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Change {
    Added,
    Removed,
    Modified,
}

enum Node {
    File(Hash),
    Directory(BTreeMap<String, Node>),
}

#[derive(Clone, Copy)]
#[repr(u8)]
enum Kind {
    File = 0,
    Directory = 1,
}

impl Snapshot {
    /// Separates path components in the text form, on every platform.
    pub const PATH_SEPARATOR: char = '/';
    /// Characters no component may hold: separators or specials on some platform.
    pub const FORBIDDEN: &[char] = &['\\', ':', '\0'];

    /// The files under `dir` named by `include` (files or directories). Entries that do not exist
    /// are left out.
    #[must_use]
    pub fn of(dir: &Path, include: &[String]) -> Self {
        let mut files = BTreeMap::new();
        for file in include.iter().flat_map(|entry| dir.join(entry).files().unwrap_or_default()) {
            let Ok(bytes) = std::fs::read(&file) else { continue };
            let relative = file.strip_prefix(dir).unwrap_or(&file);
            let components = relative
                .components()
                .filter_map(|c| match c {
                    Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
                    _ => None,
                })
                .collect();
            files.insert(components, Hash::of(&bytes));
        }
        Self { files }
    }

    /// The files by path components, in path order.
    pub fn files(&self) -> impl Iterator<Item = (&[String], Hash)> {
        self.files.iter().map(|(path, &hash)| (path.as_slice(), hash))
    }

    /// The tree hash.
    #[must_use]
    pub fn root(&self) -> Hash {
        let mut root = Node::Directory(BTreeMap::new());
        for (path, &leaf) in &self.files {
            root.insert(path, leaf);
        }
        root.hash()
    }

    /// The files that differ from `self` to `other`, by path, in path order.
    #[must_use]
    pub fn changes(&self, other: &Self) -> Vec<(String, Change)> {
        let mut changes: Vec<(&Vec<String>, Change)> = Vec::new();
        for (path, hash) in &self.files {
            match other.files.get(path) {
                None => changes.push((path, Change::Removed)),
                Some(h) if h != hash => changes.push((path, Change::Modified)),
                Some(_) => {}
            }
        }
        changes
            .extend(other.files.keys().filter(|p| !self.files.contains_key(*p)).map(|p| (p, Change::Added)));
        changes.sort();
        changes.into_iter().map(|(p, c)| (Self::path(p), c)).collect()
    }

    /// Reads the text form: one `hash path` line per file. Every component must be a plain name,
    /// so that a snapshot from an untrusted archive cannot name a file outside its tree.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut files = BTreeMap::new();
        for line in text.lines() {
            let (hash, path) = line.split_once(' ')?;
            let components: Vec<String> = path.split(Self::PATH_SEPARATOR).map(str::to_owned).collect();
            let plain = |c: &String| {
                !c.contains(Self::FORBIDDEN)
                    && matches!(Path::new(c).components().next(), Some(Component::Normal(n)) if n == c.as_str())
            };
            if !components.iter().all(plain) {
                return None;
            }
            files.insert(components, Hash::parse(hash)?);
        }
        Some(Self { files })
    }

    fn path(components: &[String]) -> String {
        components.join(&Self::PATH_SEPARATOR.to_string())
    }
}

impl fmt::Display for Snapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.files.iter().try_for_each(|(path, hash)| writeln!(f, "{hash} {}", Self::path(path)))
    }
}

impl Node {
    fn insert(&mut self, components: &[String], leaf: Hash) {
        let Self::Directory(entries) = self else { return };
        match components {
            [] => {}
            [name] => {
                entries.insert(name.clone(), Self::File(leaf));
            }
            [name, rest @ ..] => {
                let child = entries.entry(name.clone()).or_insert_with(|| Self::Directory(BTreeMap::new()));
                child.insert(rest, leaf);
            }
        }
    }

    fn hash(&self) -> Hash {
        match self {
            Self::File(leaf) => *leaf,
            Self::Directory(entries) => {
                let mut hasher = blake3::Hasher::new_derive_key(DIRECTORY_CONTEXT);
                for (name, child) in entries {
                    let kind = match child {
                        Self::File(_) => Kind::File,
                        Self::Directory(_) => Kind::Directory,
                    };
                    hasher.update(&[kind as u8]);
                    hasher.update(&(name.len() as u64).to_le_bytes());
                    hasher.update(name.as_bytes());
                    hasher.update(&child.hash().digest);
                }
                Hash { algorithm: Algorithm::Blake3, digest: *hasher.finalize().as_bytes() }
            }
        }
    }
}

impl Hash {
    /// The tree hash of the files under `dir` named by `include`; see [`Snapshot::of`].
    #[must_use]
    pub fn tree(dir: &Path, include: &[String]) -> Self {
        Snapshot::of(dir, include).root()
    }
}
