//! Roots and collection. A project registers its directory under `roots/`, named by the hash of the
//! path; its lockfile names the entries it keeps. Collection removes every entry no registered
//! lockfile records, forgets roots whose lockfile is gone, and empties staging.

use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};

use vun_fs::{Edit, Tree};
use vun_hash::{Algorithm, Hash};

use super::Store;
use crate::lock::{self, Lock};

/// A stored package, as listed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
    pub hash: Hash,
    pub label: Option<String>,
    pub bytes: u64,
    /// Whether a registered lockfile records it.
    pub rooted: bool,
}

/// One line: the hash, the size in bytes, `kept` when a lockfile records it, and the label.
impl std::fmt::Display for Listed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let rooted = if self.rooted { "kept" } else { "-" };
        write!(f, "{} {:>12} {rooted:<4} {}", self.hash, self.bytes, self.label.as_deref().unwrap_or("-"))
    }
}

/// What collection removed, and how many entries it kept.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Collected {
    pub removed: Vec<Listed>,
    pub kept: usize,
}

impl Store {
    /// Registers the project in `dir`: the entries its lockfile records are kept.
    ///
    /// # Errors
    /// Returns an error if the directory cannot be resolved or the root written.
    pub fn register(&self, dir: &Path) -> io::Result<()> {
        let dir = dir.canonicalize()?;
        let text = dir.to_string_lossy();
        self.root_of(&text).put(text.as_bytes())
    }

    fn root_of(&self, dir: &str) -> PathBuf {
        self.root.join(Self::ROOTS).join(Hash::of(dir.as_bytes()).digest())
    }

    /// Every entry, in hash order, labeled, with its size and whether a root keeps it.
    ///
    /// # Errors
    /// Returns an error if the store cannot be read.
    pub fn list(&self) -> io::Result<Vec<Listed>> {
        let live = self.live()?;
        let mut listed = Vec::new();
        for hash in self.hashes()? {
            let label = std::fs::read_to_string(self.label(hash)).ok().map(|l| l.trim().to_owned());
            let bytes = self.entry(hash).size()?;
            listed.push(Listed { hash, label, bytes, rooted: live.contains(&hash) });
        }
        Ok(listed)
    }

    /// Removes every entry no registered lockfile records, and everything staged.
    ///
    /// # Errors
    /// Returns an error if the store cannot be read or an entry removed.
    pub fn collect(&self) -> io::Result<Collected> {
        let mut collected = Collected::default();
        for listed in self.list()? {
            if listed.rooted {
                collected.kept += 1;
            } else {
                self.entry(listed.hash).remove()?;
                collected.removed.push(listed);
            }
        }
        self.root.join(Self::STAGING).remove()?;
        Ok(collected)
    }

    /// The hashes the registered lockfiles record; a root whose project has no readable lockfile
    /// is forgotten.
    fn live(&self) -> io::Result<HashSet<Hash>> {
        let mut live = HashSet::new();
        let roots = self.root.join(Self::ROOTS);
        for root in roots.entries().unwrap_or_default() {
            let project = std::fs::read_to_string(&root)?;
            let lock = std::fs::read_to_string(Path::new(&project).join(lock::FILE)).ok();
            match lock.and_then(|text| Lock::parse(&text)) {
                Some(lock) => live.extend(lock.entries().iter().map(|e| e.hash)),
                None => std::fs::remove_file(&root)?,
            }
        }
        Ok(live)
    }

    /// The hashes of the entries present, in order.
    fn hashes(&self) -> io::Result<Vec<Hash>> {
        let mut hashes = Vec::new();
        for &algorithm in Algorithm::ALL {
            for first in self.root.join(algorithm.name()).entries().unwrap_or_default() {
                for rest in first.entries()? {
                    let digest =
                        [&first, &rest].map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()));
                    if let [Some(first), Some(rest)] = digest {
                        hashes.extend(Hash::from_digest(algorithm, &(first + &rest)));
                    }
                }
            }
        }
        Ok(hashes)
    }
}
