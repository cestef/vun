//! Finding packages: the package of a file, the project around a directory, and the key that
//! names a file the same way in every process.

use std::path::{Path, PathBuf};

use vun_build::layout::MANIFEST;
use vun_build::manifest;
use vun_diag::{Diagnostic, Msg, errors};
use vun_hir::db::Scope;
use vun_span::{FileId, FileRef, SourceMap};

use super::PackageKey;
use crate::Session;

impl Session {
    /// The package `file` belongs to. The first one found is the project, whose graph version
    /// selection runs over.
    pub fn package_of(&mut self, file: FileId) -> PackageKey {
        let path = PathBuf::from(&self.sources.file(file).path);
        let found = path.parent().and_then(|dir| self.package_around(dir)).map(|(_, key)| key);
        let key = found.unwrap_or_else(|| self.loose());
        if self.packages.root.is_none() {
            self.packages.root = found;
        }
        key
    }

    /// The names a file in `dir` sees from outside itself: its package's, its dependencies'
    /// and its prelude's (spec 01, section 8), which a declaration there may not shadow.
    pub fn names_seen_in(&mut self, dir: &Path) -> Vec<String> {
        let key = self.package_around(dir).map_or_else(|| self.loose(), |(_, key)| key);
        let id = self.publish(key);
        let scope = self.db.get::<Scope>(&id);
        scope.names.keys().map(|n| n.as_str().to_owned()).collect()
    }

    /// The package whose directory is `dir` or the nearest one above it.
    pub(crate) fn package_around(&mut self, dir: &Path) -> Option<(PathBuf, PackageKey)> {
        dir.ancestors().find_map(|d| self.package_at(d).map(|key| (d.to_path_buf(), key)))
    }

    /// The file at `path` as every process names it: by its package and its path inside it, or
    /// by its path outside any package.
    pub(crate) fn file_key(&mut self, path: &Path) -> FileRef {
        let around = path.parent().and_then(|dir| self.package_around(dir));
        around.map_or_else(
            || FileRef::loose(&path.display().to_string()),
            |(_, key)| self.packages.list[&key].file(path),
        )
    }

    /// The project around the current directory; its absence is reported.
    ///
    /// # Errors
    /// Returns an error if the current directory cannot be read.
    pub fn project(&mut self) -> std::io::Result<Option<(PathBuf, PackageKey)>> {
        let found = self.package_around(&std::env::current_dir()?);
        if let Some((_, key)) = &found {
            self.packages.root = Some(*key);
        } else {
            let msg = Msg::new().text("no ").code(MANIFEST).text(" declaring ").code(manifest::PACKAGE);
            let msg = msg.text(" in this directory or above");
            let help = Msg::new()
                .text("run this in a package, or create one: a ")
                .code(MANIFEST)
                .text(" declaring ")
                .code(manifest::PACKAGE);
            self.diag.error(Diagnostic::error(errors::PACKAGE_NO_PROJECT, msg).help(help));
        }
        Ok(found)
    }
}
