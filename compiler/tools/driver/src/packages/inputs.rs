//! The query inputs of loaded packages (`vun_hir::db`): each package's description, with its
//! dependencies resolved but not reported, and the text of its source files.

use std::path::Path;
use std::sync::Arc;

use vun_build::layout::SEPARATOR;
use vun_diag::DiagCtxt;
use vun_hash::{Hash, Snapshot, Taken};
use vun_hir::db::{
    DepData, FileRef, FileText, Package as PackageInput, PackageData, PackageId, Parsed, Unloaded,
};
use vun_intern::Symbol;
use vun_par::Parallel;
use vun_span::EXTENSION;

use super::deps::Unfetched;
use super::locate::VERSION_MARK;
use super::{Package, PackageKey};
use crate::Session;

/// What stands for the files outside any package in the queries.
const LOOSE: &[u8] = b"loose";

impl PackageKey {
    #[must_use]
    pub fn id(self) -> PackageId {
        match self {
            Self::Content(hash) => PackageId(hash),
            Self::Loose => PackageId(Hash::of(LOOSE)),
        }
    }
}

impl Package {
    /// What names its files in every process: `name@version`, or empty for loose files.
    pub(super) fn key(&self) -> String {
        match (self.name, self.release) {
            (Some(name), Some((_, version))) => format!("{name}{VERSION_MARK}{version}"),
            _ => String::new(),
        }
    }

    /// The path of `file` as its queries name it: relative to its directory, components joined
    /// as in snapshots, or the whole path for a loose file.
    pub(crate) fn relative(&self, file: &Path) -> String {
        let inside = file.strip_prefix(&self.dir).ok().filter(|_| self.name.is_some());
        let parts = inside.map(|p| p.iter().map(|c| c.to_string_lossy().into_owned()).collect::<Vec<_>>());
        parts.map_or_else(|| file.display().to_string(), |p| p.join(&Snapshot::PATH_SEPARATOR.to_string()))
    }
}

/// A package's source files, by their path relative to its directory, components joined as in
/// snapshots, with the text they were hashed with: `None` for one that is not UTF-8.
#[derive(Default)]
pub(super) struct Sources(Vec<(String, Option<Arc<str>>)>);

impl Sources {
    pub(super) fn of(taken: &Taken) -> Self {
        let sources = taken.files().filter(|(path, _, _)| {
            path.last().is_some_and(|f| Path::new(f).extension().is_some_and(|e| e == EXTENSION))
        });
        let sources = sources.map(|(path, _, bytes)| {
            let text = std::str::from_utf8(bytes).ok().map(Arc::from);
            (path.join(&Snapshot::PATH_SEPARATOR.to_string()), text)
        });
        Self(sources.collect())
    }
}

impl Session {
    /// Parses every file of every package published, in parallel, when no saved memos were
    /// loaded: a first compile reads most of them, and one at a time as names reach them
    /// otherwise. With memos loaded, only the files a check reaches are parsed.
    pub(crate) fn parse_ahead(&self) {
        if self.db.restored() {
            return;
        }
        let files: Vec<FileRef> = self
            .packages
            .published
            .iter()
            .flat_map(|package| {
                let id = package.id();
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
        let id = package.id();
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
            key: p.key(),
            deps,
            prelude,
            prelude_span: p.prelude_span,
            files: p.sources.0.iter().map(|(path, _)| path.clone()).collect(),
        };
        for (path, text) in &p.sources.0 {
            self.db.set::<FileText>(&FileRef { package: id, path: Symbol::intern(path) }, text.clone());
        }
        self.db.set::<PackageInput>(&id, Arc::new(data));
        id
    }
}
