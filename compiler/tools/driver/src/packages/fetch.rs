//! Filling the project's store with its stored dependencies (spec 12, section 3): each is
//! downloaded from its mirrors in order, unpacked under staging, verified against its hash, and
//! moved into its entry with the snapshot it was verified with.

use std::error::Error;
use std::path::{Path, PathBuf};
use std::{fmt, fs, io};

use vun_build::layout::MANIFEST;
use vun_build::manifest::{DepKind, Identity, Version};
use vun_build::store::{Label, Store};
use vun_diag::{ChildKind, Diagnostic, Msg, errors};
use vun_fetch::{Failure, Mirror};
use vun_fs::{Edit, Tree};
use vun_hash::{Hash, Snapshot};
use vun_intern::Symbol;
use vun_uri::{Invalid, Uri};

use super::{Dep, DepSource, PackageKey};
use crate::Session;

/// What fetching did for one stored dependency.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fetched {
    /// Already in the store, matching the snapshot it was verified with.
    Present,
    /// Downloaded from this mirror and verified.
    Stored { url: Uri },
    /// No mirror served it; reported.
    Failed,
}

/// Why the archive of one mirror was not stored.
#[derive(Debug)]
pub(super) enum Rejected {
    /// Not a URL (RFC 3986).
    Url(Invalid),
    Mirror(Failure),
    /// The unpacked tree hashes to another package.
    Hash {
        found: Hash,
    },
    /// What it unpacked cannot be read as a package, such as a link or a path outside it.
    Unpacked(io::Error),
    /// It does not declare the release an index lists it as.
    Release {
        identity: Identity,
        version: Version,
    },
    Store(io::Error),
}

impl fmt::Display for Rejected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Url(_) => f.write_str("not a URL"),
            Self::Mirror(failure) => failure.fmt(f),
            Self::Hash { found } => write!(f, "its contents hash to `{found}`"),
            Self::Unpacked(_) => f.write_str("cannot read what it unpacked"),
            Self::Release { identity, version } => {
                write!(f, "it does not declare the release `{identity}` `{version}` it is listed as")
            }
            Self::Store(_) => f.write_str("cannot move it into the store"),
        }
    }
}

impl Error for Rejected {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Url(e) => Some(e),
            Self::Mirror(failure) => failure.source(),
            Self::Hash { .. } | Self::Release { .. } => None,
            Self::Unpacked(e) | Self::Store(e) => Some(e),
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
        // The declared graph, not the selected one: selection reads the lockfile this fills.
        self.walk(project, |s, _, dep| {
            let fetched = s.obtain(dep, &mut done);
            (fetched != Some(Fetched::Failed) && dep.kind == DepKind::Vun)
                .then(|| s.dep_package(dep).ok())
                .flatten()
        });
        done
    }

    /// Stores `dep` if it is stored and the store lacks it, once per hash, recording what
    /// happened in `done`; `None` for a directory or a hash already handled.
    fn obtain(&mut self, dep: &Dep, done: &mut Vec<(Symbol, Hash, Fetched)>) -> Option<Fetched> {
        let DepSource::Stored { hash, version, urls } = &dep.source else { return None };
        if done.iter().any(|(_, h, _)| h == hash) {
            return None;
        }
        let outcome = if self.present(dep.kind, *hash) {
            Fetched::Present
        } else {
            self.download(dep, *hash, *version, urls)
        };
        done.push((dep.name, *hash, outcome.clone()));
        Some(outcome)
    }

    /// Whether the store holds `hash` as it was verified: a package read as the session reads
    /// every package, once, or a foreign tree against the snapshot recorded with it.
    fn present(&mut self, kind: DepKind, hash: Hash) -> bool {
        let store = self.store.clone();
        if kind == DepKind::Vun {
            return self.package_at(&store.package(hash)) == Some(PackageKey::Content(hash));
        }
        let Some(recorded) = fs::read_to_string(store.snapshot(hash)).ok().and_then(|t| Snapshot::parse(&t))
        else {
            return false;
        };
        let mut top: Vec<String> = recorded.files().map(|(path, _)| path[0].clone()).collect();
        top.dedup();
        Snapshot::of(&store.package(hash), &top).is_ok_and(|found| found == recorded)
    }

    fn download(&mut self, dep: &Dep, hash: Hash, version: Version, urls: &[Uri]) -> Fetched {
        let store = self.store.clone();
        let name = dep.name.as_str();
        let msg = Msg::new().text("cannot fetch ").code(name).text(" (").code(hash.to_string()).text(")");
        let staging = match store.stage(hash) {
            Ok(staging) => staging,
            Err(e) => {
                self.report(dep.span, Diagnostic::error(errors::PACKAGE_FETCH_FAILED, msg).caused_by(&e));
                return Fetched::Failed;
            }
        };
        let label = Label { name, version };
        let mut rejected = Vec::new();
        for url in urls {
            match self.stage(dep.kind, &store, hash, &staging, url, label) {
                Ok(()) => {
                    self.packages.wrote(&store.package(hash).join(MANIFEST));
                    return Fetched::Stored { url: url.clone() };
                }
                Err(why) => rejected.push((url, why)),
            }
        }
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
    pub(super) fn stage(
        &mut self,
        kind: DepKind,
        store: &Store,
        hash: Hash,
        staging: &Path,
        url: &Uri,
        label: Label<'_>,
    ) -> Result<(), Rejected> {
        let mirror = Mirror::of(url, &self.registry.schemes()).map_err(Rejected::Mirror)?;
        let unpacked = Self::unpack(staging, &mirror)?;
        let snapshot = Snapshot::of(&unpacked, &self.include(kind, &unpacked)).map_err(Rejected::Unpacked)?;
        if snapshot.root() != hash {
            return Err(Rejected::Hash { found: snapshot.root() });
        }
        Self::commit(store, staging, &snapshot, label)
    }

    /// Downloads and unpacks `mirror` into a fresh `staging` entry, returning where the package
    /// is.
    pub(super) fn unpack(staging: &Path, mirror: &Mirror) -> Result<PathBuf, Rejected> {
        staging.fresh().map_err(Rejected::Store)?;
        let unpacked = staging.join(Store::PACKAGE);
        mirror.fetch(&unpacked).map_err(Rejected::Mirror)?;
        Ok(unpacked)
    }

    /// Moves the verified `staging` entry into the store under the root of `snapshot`, recording
    /// the snapshot it was verified with and its label; an entry another process stored first is
    /// kept, as it holds the same contents.
    pub(super) fn commit(
        store: &Store,
        staging: &Path,
        snapshot: &Snapshot,
        label: Label<'_>,
    ) -> Result<(), Rejected> {
        let commit = || -> io::Result<()> {
            staging.join(Store::SNAPSHOT).put(snapshot.to_string())?;
            staging.join(Store::LABEL).put(label.to_string())?;
            store.entry(snapshot.root()).install(staging)
        };
        commit().map_err(Rejected::Store)
    }

    /// What the hash of an unpacked package covers: its manifest's `include`, or everything in
    /// it for a foreign package.
    pub(super) fn include(&mut self, kind: DepKind, dir: &Path) -> Vec<String> {
        let manifest = (kind == DepKind::Vun).then(|| self.manifest_at(&dir.join(MANIFEST))).flatten();
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
