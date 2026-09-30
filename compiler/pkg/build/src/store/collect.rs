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

/// What collection removed, and how many entries it kept.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Collected {
    pub removed: Vec<Listed>,
    pub kept: usize,
}

/// A registered lockfile that does not parse.
#[derive(Debug)]
pub struct Unparsed(pub PathBuf);

impl std::fmt::Display for Unparsed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "`{}` is not a lockfile", self.0.display())
    }
}

impl std::error::Error for Unparsed {}

impl Store {
    /// Registers the project in `dir`: the entries its lockfile records are kept.
    ///
    /// # Errors
    /// Returns an error if the directory cannot be resolved or the root written.
    pub fn register(&self, dir: &Path) -> io::Result<()> {
        let dir = dir.resolved()?;
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
        // Alone in the store: no fetch is staging, or reading an entry it is about to record.
        let held = self.lock()?;
        held.lock()?;
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

    /// The hashes the registered lockfiles record; a root whose project has no lockfile any more
    /// is forgotten. A lockfile that cannot be read or does not parse stops collection, which
    /// could otherwise remove what it records.
    fn live(&self) -> io::Result<HashSet<Hash>> {
        let mut live = HashSet::new();
        let roots = self.root.join(Self::ROOTS);
        for root in roots.entries().unwrap_or_default() {
            let project = std::fs::read_to_string(&root)?;
            let path = Path::new(&project).join(lock::FILE);
            let text = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    std::fs::remove_file(&root)?;
                    continue;
                }
                Err(e) => return Err(e),
            };
            let lock = Lock::parse(&text)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, Unparsed(path)))?;
            live.extend(lock.entries().iter().map(|e| e.hash));
        }
        Ok(live)
    }

    /// The hashes of the entries present, in order.
    fn hashes(&self) -> io::Result<Vec<Hash>> {
        let mut hashes = Vec::new();
        for &algorithm in Algorithm::ALL {
            let firsts = self.root.join(algorithm.name()).entries().unwrap_or_default();
            for first in firsts.into_iter().filter(|p| p.is_dir()) {
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
