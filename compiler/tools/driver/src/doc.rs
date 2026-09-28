//! Documenting packages (spec 12, section 7): the namespaces of a package's files read into
//! `vun-doc`'s pages, each declaration of a type told from a value by what it declares.

use std::io;
use std::path::Path;

use vun_build::layout::{Location, MANIFEST};
use vun_diag::Diagnostic;
use vun_doc::Docs;
use vun_fs::Tree;
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

    /// Adds the namespaces of the package in `dir` to `docs`: each source file but the manifest,
    /// its declarations of types told from its values by their types.
    ///
    /// # Errors
    /// Fails when the directory cannot be walked.
    pub fn document(&mut self, docs: &mut Docs, dir: &Path) -> io::Result<()> {
        for file in dir.files_with(EXTENSION)? {
            let Some(path) = Location::path_of(dir, &file) else { continue };
            if file.parent() == Some(dir) && file.file_name().is_some_and(|name| name == MANIFEST) {
                continue;
            }
            match std::fs::read_to_string(&file) {
                Ok(text) => {
                    docs.add(path.clone(), file.display().to_string(), &text);
                    let loaded = self.open(&file, text);
                    let key = self.file_ref(loaded);
                    let Some(namespace) = docs.namespaces.get_mut(&path) else { continue };
                    namespace.retype(|name| {
                        let declares = self.db.get::<Declaration>(&(key.clone(), Symbol::intern(name)));
                        declares == Some(Declares::Type)
                    });
                }
                Err(e) => {
                    self.diag.error(Diagnostic::unreadable(&file, &e));
                }
            }
        }
        Ok(())
    }
}
