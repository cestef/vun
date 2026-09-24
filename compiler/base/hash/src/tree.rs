//! Tree hashes: a Merkle tree over directory entries. A file's node is the hash of its contents; a
//! directory's is the hash of its entries in name order, each a kind (file, executable file or
//! directory), a name and the child's hash.
//! Names are single path components, so a tree hashes the same on every platform, and a file's
//! hash is the one every cache already computes.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Component, Path};

use vun_fs::{Mode, Modes, Tree};

use super::{Algorithm, Hash};

/// Separates directory nodes from file contents, so that no file hashes like a directory.
const DIRECTORY_CONTEXT: &str = "vun 2026 directory node";

/// The hash of every file of a tree, by its path, from which the tree's hash is computed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    files: BTreeMap<Vec<String>, Entry>,
}

/// A file of a snapshot: the hash of its contents and its mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub hash: Hash,
    pub mode: Mode,
}

/// How a file differs between two snapshots.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Change {
    Added,
    Removed,
    Modified,
    /// Same contents, made executable or no longer executable.
    Mode,
}

enum Node {
    File(Entry),
    Directory(BTreeMap<String, Node>),
}

#[derive(Clone, Copy)]
#[repr(u8)]
enum Kind {
    File = 0,
    Directory = 1,
    Executable = 2,
}

/// How each mode is marked in the text form.
const MODES: [(char, Mode); 2] = [('-', Mode::Regular), ('x', Mode::Executable)];

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
            files.insert(components, Entry { hash: Hash::of(&bytes), mode: file.mode() });
        }
        Self { files }
    }

    /// The files by path components, in path order.
    pub fn files(&self) -> impl Iterator<Item = (&[String], Entry)> {
        self.files.iter().map(|(path, &entry)| (path.as_slice(), entry))
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
        for (path, entry) in &self.files {
            match other.files.get(path) {
                None => changes.push((path, Change::Removed)),
                Some(e) if e.hash != entry.hash => changes.push((path, Change::Modified)),
                Some(e) if e.mode != entry.mode => changes.push((path, Change::Mode)),
                Some(_) => {}
            }
        }
        changes
            .extend(other.files.keys().filter(|p| !self.files.contains_key(*p)).map(|p| (p, Change::Added)));
        changes.sort();
        changes.into_iter().map(|(p, c)| (Self::path(p), c)).collect()
    }

    /// Reads the text form: one `hash mode path` line per file, the mode `-` or `x` (executable).
    /// Every component must be a plain name, so that a snapshot from an untrusted archive cannot
    /// name a file outside its tree.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut files = BTreeMap::new();
        for line in text.lines() {
            let (hash, rest) = line.split_once(' ')?;
            let (mark, path) = rest.split_once(' ')?;
            let mut marks = mark.chars();
            let mark = marks.next().filter(|_| marks.next().is_none())?;
            let mode = MODES.iter().find(|&&(m, _)| m == mark)?.1;
            files.insert(Self::components(path)?, Entry { hash: Hash::parse(hash)?, mode });
        }
        Some(Self { files })
    }

    /// The components of `path`, written with [`Self::PATH_SEPARATOR`], if each is a plain name:
    /// not empty, `.` or `..`, and holding no [`Self::FORBIDDEN`] character. Paths from untrusted
    /// archives go through it, so that none names a file outside its tree.
    #[must_use]
    pub fn components(path: &str) -> Option<Vec<String>> {
        let plain = |c: &str| {
            !c.contains(Self::FORBIDDEN)
                && matches!(Path::new(c).components().next(), Some(Component::Normal(n)) if n == c)
        };
        path.split(Self::PATH_SEPARATOR).map(|c| plain(c).then(|| c.to_owned())).collect()
    }

    fn path(components: &[String]) -> String {
        components.join(&Self::PATH_SEPARATOR.to_string())
    }
}

impl fmt::Display for Snapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.files.iter().try_for_each(|(path, entry)| {
            let mark = MODES.iter().find(|&&(_, m)| m == entry.mode).map_or('-', |&(c, _)| c);
            writeln!(f, "{} {mark} {}", entry.hash, Self::path(path))
        })
    }
}

impl Node {
    fn insert(&mut self, components: &[String], leaf: Entry) {
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
            Self::File(leaf) => leaf.hash,
            Self::Directory(entries) => {
                let mut hasher = blake3::Hasher::new_derive_key(DIRECTORY_CONTEXT);
                for (name, child) in entries {
                    let kind = match child {
                        Self::File(Entry { mode: Mode::Regular, .. }) => Kind::File,
                        Self::File(Entry { mode: Mode::Executable, .. }) => Kind::Executable,
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
