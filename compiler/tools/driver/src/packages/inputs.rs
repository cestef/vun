//! The query inputs of loaded packages (`overt_hir::db`): each package's description, with its
//! dependencies resolved but not reported, and the text of its source files.

use std::path::Path;
use std::sync::Arc;

use overt_build::layout::SEPARATOR;
use overt_diag::DiagCtxt;
use overt_hash::{Hash, Snapshot};
use overt_hir::db::{DepData, FileRef, FileText, Package as PackageInput, PackageData, PackageId, Unloaded};
use overt_intern::Symbol;
use overt_span::EXTENSION;

use super::locate::VERSION_MARK;
use super::scope::Unfetched;
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

    /// Its source files, relative to its directory, components joined as in snapshots.
    fn sources(&self) -> Vec<String> {
        let files = self.snapshot.files().map(|(path, _)| path);
        let sources = files.filter(|path| {
            path.last().is_some_and(|f| Path::new(f).extension().is_some_and(|e| e == EXTENSION))
        });
        sources.map(|path| path.join(&Snapshot::PATH_SEPARATOR.to_string())).collect()
    }
}

impl Session {
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
            files: p.sources(),
        };
        for path in &data.files {
            let text = std::fs::read_to_string(p.dir.join(path)).ok();
            self.db.set::<FileText>(FileRef { package: id, path: path.clone() }, text.map(Arc::from));
        }
        self.db.set::<PackageInput>(id, Arc::new(data));
        id
    }
}
