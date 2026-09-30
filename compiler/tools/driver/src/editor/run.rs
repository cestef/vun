//! What runs: the declarations of a root file exporting the program it builds, where an editor
//! offers to run it.

use vun_hir::db::Parsed;
use vun_origin::Resolve;
use vun_span::{FileId, Span};

use crate::Session;

impl Session {
    /// The name of each declaration of `file` exporting the program rooted at it.
    pub fn runnable(&mut self, file: FileId) -> Vec<Span> {
        let program = self.program(file);
        program
            .roots
            .iter()
            .filter_map(|decl| {
                let parsed = self.db.get::<Parsed>(&decl.file)?;
                let hir = &parsed.hir;
                Some(self.source_span(hir.pats[hir.decls[decl.decl.id].pat].origin))
            })
            .filter(|span| span.file == file)
            .collect()
    }
}
