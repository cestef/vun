//! Filling the project's store with its stored dependencies (spec 12, section 3): each is
//! downloaded from its mirrors in order, unpacked under staging, verified against its hash, and
//! moved into its entry with the snapshot it was verified with.

use std::collections::HashSet;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::{fmt, fs, io};

use overt_build::layout::MANIFEST;
use overt_build::manifest::{DepKind, Version};
use overt_build::store::{Label, Store};
use overt_diag::{ChildKind, Diagnostic, Msg, errors};
use overt_fs::{Edit, Tree};
use overt_hash::{Hash, Snapshot};
use overt_intern::Symbol;
use overt_pkg::{Failure, Mirror};

use super::{Dep, DepSource, PackageKey};
use crate::Session;

/// What fetching did for one stored dependency.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fetched {
    /// Already in the store, matching the snapshot it was verified with.
    Present,
    /// Downloaded from this mirror and verified.
    Stored { url: String },
    /// No mirror served it; reported.
    Failed,
}

/// Why the archive of one mirror was not stored.
#[derive(Debug)]
pub(super) enum Rejected {
    Mirror(Failure),
    /// The unpacked tree hashes to another package.
    Hash {
        found: Hash,
    },
    Store(io::Error),
}

impl fmt::Display for Rejected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mirror(failure) => failure.fmt(f),
            Self::Hash { found } => write!(f, "its contents hash to `{found}`"),
            Self::Store(_) => f.write_str("cannot move it into the store"),
        }
    }
}

impl Error for Rejected {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Mirror(failure) => failure.source(),
            Self::Hash { .. } => None,
            Self::Store(e) => Some(e),
        }
    }
}

impl Session {
    /// Fetches every stored dependency reachable from `project` that the store lacks, returning
    /// what happened to each, by name and hash, in the order they were reached.
    pub fn fetch(&mut self, project: PackageKey) -> Vec<(Symbol, Hash, Fetched)> {
        let mut done: Vec<(Symbol, Hash, Fetched)> = Vec::new();
        // Replacements first: the graph is walked through them.
        for replace in self.packages.list[&project].replace.clone() {
            self.obtain(&replace.dep, &mut done);
        }
        let mut seen = HashSet::new();
        let mut pending = vec![project];
        while let Some(package) = pending.pop() {
            if !seen.insert(package) {
                continue;
            }
            let deps = self.packages.list[&package].deps.clone();
            for (index, dep) in deps.iter().enumerate() {
                if self.obtain(dep, &mut done) != Some(Fetched::Failed)
                    && dep.kind == DepKind::Overt
                    && let Some(next) = self.dep(package, index)
                {
                    pending.push(next);
                }
            }
        }
        done
    }

    /// Stores `dep` if it is stored and the store lacks it, once per hash, recording what
    /// happened in `done`; `None` for a directory or a hash already handled.
    fn obtain(&mut self, dep: &Dep, done: &mut Vec<(Symbol, Hash, Fetched)>) -> Option<Fetched> {
        let DepSource::Stored { hash, version, urls } = &dep.source else { return None };
        if done.iter().any(|(_, h, _)| h == hash) {
            return None;
        }
        let outcome =
            if self.present(*hash) { Fetched::Present } else { self.download(dep, *hash, *version, urls) };
        done.push((dep.name, *hash, outcome.clone()));
        Some(outcome)
    }

    /// Whether the store holds `hash` as it was verified.
    fn present(&self, hash: Hash) -> bool {
        let store = &self.store;
        let Some(recorded) = fs::read_to_string(store.snapshot(hash)).ok().and_then(|t| Snapshot::parse(&t))
        else {
            return false;
        };
        let mut top: Vec<String> = recorded.files().map(|(path, _)| path[0].clone()).collect();
        top.dedup();
        Snapshot::of(&store.package(hash), &top) == recorded
    }

    fn download(&mut self, dep: &Dep, hash: Hash, version: Version, urls: &[String]) -> Fetched {
        let store = self.store.clone();
        let staging = store.staging(hash);
        let name = dep.name.as_str().to_owned();
        let label = Label { name: &name, version };
        let mut rejected = Vec::new();
        for url in urls {
            match self.stage(dep.kind, &store, hash, &staging, url, label) {
                Ok(()) => return Fetched::Stored { url: url.clone() },
                Err(why) => rejected.push((url, why)),
            }
        }
        let _ = staging.remove();
        let name = dep.name.as_str();
        let msg = Msg::new().text("cannot fetch ").code(name).text(" (").code(hash.to_string()).text(")");
        let mut d = Diagnostic::error(errors::PACKAGE_FETCH_FAILED, msg);
        if urls.is_empty() {
            d = d.child(ChildKind::Help, Msg::new().text("declare mirrors in its ").code(".urls"));
        }
        for (url, why) in rejected {
            d = d.child(ChildKind::Note, Msg::new().text("from ").code(url.as_str())).caused_by(&why);
        }
        self.report(dep.span, d);
        Fetched::Failed
    }

    /// Downloads `url` under `staging`, verifies it against `hash` and moves it into the store.
    fn stage(
        &mut self,
        kind: DepKind,
        store: &Store,
        hash: Hash,
        staging: &Path,
        url: &str,
        label: Label<'_>,
    ) -> Result<(), Rejected> {
        let unpacked = Self::unpack(staging, url)?;
        let snapshot = Snapshot::of(&unpacked, &self.include(kind, &unpacked));
        if snapshot.root() != hash {
            return Err(Rejected::Hash { found: snapshot.root() });
        }
        Self::commit(store, staging, &snapshot, label)
    }

    /// Downloads and unpacks `url` into a fresh `staging` entry, returning where the package is.
    pub(super) fn unpack(staging: &Path, url: &str) -> Result<PathBuf, Rejected> {
        staging.fresh().map_err(Rejected::Store)?;
        let unpacked = staging.join(Store::PACKAGE);
        Mirror::parse(url).and_then(|m| m.fetch(&unpacked)).map_err(Rejected::Mirror)?;
        Ok(unpacked)
    }

    /// Moves the verified `staging` entry into the store under the root of `snapshot`, recording
    /// the snapshot it was verified with and its label.
    pub(super) fn commit(
        store: &Store,
        staging: &Path,
        snapshot: &Snapshot,
        label: Label<'_>,
    ) -> Result<(), Rejected> {
        let commit = || -> io::Result<()> {
            staging.join(Store::SNAPSHOT).put(snapshot.to_string())?;
            staging.join(Store::LABEL).put(label.to_string())?;
            store.entry(snapshot.root()).replace_with(staging)
        };
        commit().map_err(Rejected::Store)
    }

    /// What the hash of an unpacked package covers: its manifest's `include`, or everything in
    /// it for a foreign package.
    pub(super) fn include(&mut self, kind: DepKind, dir: &Path) -> Vec<String> {
        let manifest = (kind == DepKind::Overt).then(|| self.manifest_at(&dir.join(MANIFEST))).flatten();
        manifest.map_or_else(
            || {
                let entries = dir.entries().unwrap_or_default();
                entries
                    .iter()
                    .filter_map(|e| e.file_name().map(|n| n.to_string_lossy().into_owned()))
                    .collect()
            },
            |m| m.include,
        )
    }
}
