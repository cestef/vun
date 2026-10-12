//! The libraries of the system a program is linked against because its packages ask for them.

use std::collections::HashSet;

use vun_intern::Symbol;
use vun_span::FileId;

use crate::Session;

impl Session {
    /// The system libraries (`.system`) of the package of `file` and of the packages it depends
    /// on, each once.
    pub(crate) fn system_libraries(&mut self, file: FileId) -> Vec<Symbol> {
        let mut seen = HashSet::new();
        let mut pending = vec![self.package_of(file)];
        let mut libraries: Vec<Symbol> = Vec::new();
        while let Some(package) = pending.pop() {
            if !seen.insert(package) {
                continue;
            }
            for &library in &self.packages.list[&package].system {
                if !libraries.contains(&library) {
                    libraries.push(library);
                }
            }
            for dep in self.packages.list[&package].deps.clone() {
                pending.extend(self.found(&dep).ok());
            }
        }
        libraries
    }
}
