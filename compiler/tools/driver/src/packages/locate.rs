//! Finding packages: the package of a file, the project around a directory, and the key that
//! names a file the same way in every process.

use std::path::{Path, PathBuf};

use overt_build::layout::MANIFEST;
use overt_build::manifest;
use overt_diag::{Diagnostic, Msg, errors};
use overt_hash::Snapshot;
use overt_span::{FileId, SourceMap};

use super::PackageKey;
use crate::Session;

/// Separates a package's name from its version in a file key.
pub(super) const VERSION_MARK: char = '@';
/// Separates the parts of a file key, on every platform.
const SEPARATOR: char = Snapshot::PATH_SEPARATOR;

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

    /// The package whose directory is `dir` or the nearest one above it.
    fn package_around(&mut self, dir: &Path) -> Option<(PathBuf, PackageKey)> {
        dir.ancestors().find_map(|d| self.package_at(d).map(|key| (d.to_path_buf(), key)))
    }

    /// The key naming the file at `path` the same way in every process and on every machine:
    /// `<package>@<version>/<path inside it>`, or the path of a file outside any package.
    pub(crate) fn file_key(&mut self, path: &Path) -> String {
        let around = path.parent().and_then(|dir| self.package_around(dir));
        let release = around.and_then(|(dir, key)| {
            let package = &self.packages.list[&key];
            let relative = path.strip_prefix(&dir).ok()?;
            let parts = relative.iter().map(|p| p.to_string_lossy().into_owned());
            let key = package.key();
            (!key.is_empty())
                .then(|| [key].into_iter().chain(parts).collect::<Vec<_>>().join(&SEPARATOR.to_string()))
        });
        release.unwrap_or_else(|| path.display().to_string())
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
            self.diag.error(Diagnostic::error(errors::PACKAGE_NO_PROJECT, msg));
        }
        Ok(found)
    }
}
