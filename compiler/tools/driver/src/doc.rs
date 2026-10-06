//! Documenting packages (spec 12, section 7): the namespaces of a package's files read into
//! `vun-doc`'s pages, each declaration of a type told from a value by what it declares.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use vun_build::layout::{Location, MANIFEST};
use vun_diag::Diagnostic;
use vun_documenter::{Documenter, Site, Source, Types};
use vun_fs::Tree;
use vun_hir::db::FileRef;
use vun_intern::Symbol;
use vun_sema::decl::{Declaration, Declares};
use vun_span::EXTENSION;

use crate::Session;

impl Session {
    /// Whether `dir` is a package of the library: it holds its root namespace's file.
    #[must_use]
    pub fn is_package_root(dir: &Path) -> bool {
        dir.file_name().is_some_and(|name| dir.join(name).with_extension(EXTENSION).is_file())
    }

    /// The directories of the library's packages: the ones at its top, and the ones in the
    /// directories that group them (`boards/`).
    #[must_use]
    pub fn library_packages(&self) -> Vec<PathBuf> {
        fn within(dir: &Path) -> Vec<PathBuf> {
            let below = dir.entries().unwrap_or_default().into_iter().filter(|d| d.is_dir());
            below.flat_map(|d| if Session::is_package_root(&d) { vec![d] } else { within(&d) }).collect()
        }
        within(&self.library)
    }

    /// The documentation `documenter` writes of the packages in `dirs`: each source file but the
    /// manifest, the declarations of types told from those of values by their types.
    ///
    /// # Errors
    /// Fails when a directory cannot be walked or the documenter fails.
    pub fn document(&mut self, documenter: &dyn Documenter, dirs: &[PathBuf]) -> io::Result<Site> {
        let mut sources = Vec::new();
        let mut keys = HashMap::new();
        for dir in dirs {
            for file in dir.files_with(EXTENSION)? {
                let Some(path) = Location::path_of(dir, &file) else { continue };
                if file.parent() == Some(dir.as_path())
                    && file.file_name().is_some_and(|name| name == MANIFEST)
                {
                    continue;
                }
                match std::fs::read_to_string(&file) {
                    Ok(text) => {
                        let loaded = self.open(&file, text.clone());
                        keys.insert(path.clone(), self.file_ref(loaded));
                        sources.push(Source { path, file: file.display().to_string(), text });
                    }
                    Err(e) => {
                        self.diag.error(Diagnostic::unreadable(&file, &e));
                    }
                }
            }
        }
        documenter.document(&sources, &Declared { session: self, keys }).map_err(io::Error::other)
    }
}

/// What a session's queries say of the declarations of the namespaces being documented.
struct Declared<'s> {
    session: &'s Session,
    keys: HashMap<Vec<String>, FileRef>,
}

impl Types for Declared<'_> {
    fn is_type(&self, path: &[String], name: &str) -> bool {
        let Some(key) = self.keys.get(path) else { return false };
        let declares = self.session.db.get::<Declaration>(&(key.clone(), Symbol::intern(name)));
        declares == Some(Declares::Type)
    }
}
