//! Tree hashes: a Merkle tree over directory entries. A file's node is the hash of its contents; a
//! directory's is the hash of its entries in name order, each a kind, a name and the child's hash.
//! Names are single path components, so a tree hashes the same on every platform, and a file's
//! hash is the one every cache already computes.

use std::collections::BTreeMap;
use std::path::{Component, Path};

use overt_fs::Tree;

use super::{Algorithm, Hash};

/// Separates directory nodes from file contents, so that no file hashes like a directory.
const DIRECTORY_CONTEXT: &str = "overt 2026 directory node";

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
    /// The tree hash of the files under `dir` named by `include` (files or directories).
    /// Entries that do not exist are left out.
    #[must_use]
    pub fn tree(dir: &Path, include: &[String]) -> Self {
        let mut root = Node::Directory(BTreeMap::new());
        for file in include.iter().flat_map(|entry| dir.join(entry).files().unwrap_or_default()) {
            let Ok(bytes) = std::fs::read(&file) else { continue };
            let relative = file.strip_prefix(dir).unwrap_or(&file);
            let components: Vec<String> = relative
                .components()
                .filter_map(|c| match c {
                    Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
                    _ => None,
                })
                .collect();
            root.insert(&components, Self::of(&bytes));
        }
        root.hash()
    }
}
