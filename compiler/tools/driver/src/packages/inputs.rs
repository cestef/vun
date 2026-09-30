//! The query inputs of loaded packages (`vun_hir::db`): each package's description, with its
//! dependencies resolved but not reported, the text of its source files and the bytes of every
//! file it holds.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use vun_build::layout::{MANIFEST, SEPARATOR};
use vun_diag::DiagCtxt;
use vun_hash::{Snapshot, Taken};
use vun_hir::db::{
    DepData, FileBytes, FileRef, FileText, Package as PackageInput, PackageData, PackageId, Parsed, Unloaded,
};
use vun_intern::Symbol;
use vun_par::Parallel;
use vun_query::Sharing;
use vun_span::{EXTENSION, FileId};

use super::deps::Unfetched;
use super::{Package, PackageKey, Packages};
use crate::Session;

impl Package {
    /// `file`, one of its files, as every process names it.
    pub(crate) fn file(&self, file: &Path) -> FileRef {
        FileRef { package: self.id, path: Symbol::intern(&self.relative(file)) }
    }

    /// The path of `file` as its queries name it: relative to its directory, components joined
    /// as in snapshots, or the whole path for a loose file.
    pub(crate) fn relative(&self, file: &Path) -> String {
        let inside = file.strip_prefix(&self.dir).ok().filter(|_| self.name.is_some());
        let parts = inside.map(|p| p.iter().map(|c| c.to_string_lossy().into_owned()).collect::<Vec<_>>());
        parts.map_or_else(|| file.display().to_string(), |p| p.join(&Snapshot::PATH_SEPARATOR.to_string()))
    }
}

/// A package's files, by their path relative to its directory, components joined as in
/// snapshots, with the bytes they were hashed with.
#[derive(Default)]
pub(super) struct Sources(Vec<(String, Arc<[u8]>)>);

impl Sources {
    pub(super) fn of(taken: &Taken) -> Self {
        let files = taken
            .files()
            .map(|(path, _, bytes)| (path.join(&Snapshot::PATH_SEPARATOR.to_string()), Arc::from(bytes)));
        Self(files.collect())
    }

    /// The source files, each with its text: `None` for one that is not UTF-8.
    fn texts(&self) -> impl Iterator<Item = (&String, Option<Arc<str>>)> {
        let sources =
            self.0.iter().filter(|(path, _)| Path::new(path).extension().is_some_and(|e| e == EXTENSION));
        sources.map(|(path, bytes)| (path, std::str::from_utf8(bytes).ok().map(Arc::from)))
    }
}

impl super::Package {
    /// Its source files, by their paths below its directory.
    pub fn files(&self) -> impl Iterator<Item = PathBuf> + '_ {
        self.sources.texts().map(|(path, _)| self.dir.join(path))
    }
}

impl Session {
    /// Parses every file of the package of `file` and those it depends on, in parallel, when no
    /// saved memos were loaded: a first compile reads most of them, and one at a time as names
    /// reach them otherwise. With memos loaded, only the files a check reaches are parsed.
    pub(crate) fn parse_ahead(&mut self, file: FileId) {
        if self.db.restored() {
            return;
        }
        let package = self.package_of(file);
        self.publish(package);
        let files: Vec<FileRef> = self
            .packages
            .published
            .iter()
            .flat_map(|package| {
                let id = self.packages.list[package].id;
                let data = self.db.get::<PackageInput>(&id);
                data.files
                    .iter()
                    .map(move |path| FileRef { package: id, path: Symbol::intern(path) })
                    .collect::<Vec<_>>()
            })
            .collect();
        let db = &self.db;
        files.par_map(|file| drop(db.get::<Parsed>(file)));
    }

    /// Sets the inputs of `package` and of every package it depends on, once each, and returns its
    /// id. Dependencies are resolved without reporting: a missing one carries its diagnostic, which
    /// the queries return where it is needed.
    pub fn publish(&mut self, package: PackageKey) -> PackageId {
        let id = self.packages.list[&package].id;
        if !self.packages.published.insert(package) {
            return id;
        }
        let mut deps = Vec::new();
        for index in 0..self.packages.list[&package].deps.len() {
            let name = self.packages.list[&package].deps[index].name;
            let target = match self.dep_unreported(package, index) {
                Ok(found) => Ok(self.publish(found)),
                Err(Unfetched::Absent(d)) => {
                    let mut reports = DiagCtxt::new();
                    reports.error(*d);
                    Err(Unloaded::Absent(reports))
                }
                Err(Unfetched::Reported(_)) => Err(Unloaded::Reported),
            };
            deps.push(DepData { name, package: target });
        }
        let p = &self.packages.list[&package];
        let prelude =
            p.prelude.iter().map(|path| path.split(SEPARATOR).map(Symbol::intern).collect()).collect();
        let data = PackageData {
            name: p.name,
            dir: p.dir.display().to_string(),
            deps,
            prelude,
            prelude_span: p.prelude_span,
            files: p.sources.texts().map(|(path, _)| path.clone()).collect(),
            manifest: p
                .sources
                .texts()
                .map(|(path, _)| path)
                .find(|&path| path == MANIFEST)
                .map(|path| Symbol::intern(path)),
        };
        // A library's inputs are the same in every project, so what reads them alone is shared.
        let sharing = if self.packages.library(package) { Sharing::Shared } else { Sharing::Own };
        for (path, text) in p.sources.texts() {
            let key = FileRef { package: id, path: Symbol::intern(path) };
            self.db.set_as::<FileText>(&key, text, sharing);
        }
        for (path, bytes) in &p.sources.0 {
            let key = FileRef { package: id, path: Symbol::intern(path) };
            self.db.set_as::<FileBytes>(&key, Some(Arc::clone(bytes)), sharing);
        }
        self.db.set_as::<PackageInput>(&id, Arc::new(data), sharing);
        id
    }
}

impl Packages {
    /// Whether `key` is a library of the project: a package other than the project and the
    /// files outside any package, whose inputs every project sees alike.
    #[must_use]
    pub fn library(&self, key: PackageKey) -> bool {
        key != PackageKey::Loose && self.root != Some(key)
    }
}
