//! What runs: the declarations of a root file exporting the program it builds and the tests a
//! file declares, where an editor offers to run them.

use std::path::PathBuf;

use vun_hir::db::Parsed;
use vun_intern::Symbol;
use vun_origin::Resolve;
use vun_sema::testing::Tests;
use vun_sema_ty::item::DeclRef;
use vun_span::{FileId, SourceMap, Span};

use crate::Session;

impl Session {
    /// The name of each declaration of `file` exporting the program rooted at it.
    pub fn runnable(&mut self, file: FileId) -> Vec<Span> {
        let program = self.program(file);
        program
            .roots
            .iter()
            .filter_map(|decl| self.name_span(decl))
            .filter(|span| span.file == file)
            .collect()
    }

    /// Each test `file` declares, by its name and where the name is written: those the runner
    /// finds in the file's namespace (`std.meta.collect`), none outside any package.
    pub fn tests(&mut self, file: FileId) -> Vec<(Symbol, Span)> {
        let path = PathBuf::from(&self.sources.file(file).path);
        let Some(ns) = self.namespace_at(file, Some(&path)) else { return Vec::new() };
        let key = self.file_ref(file);
        let Ok(tests) = self.db.get::<Tests>(&(key, ns)) else { return Vec::new() };
        let spans = tests.iter().filter_map(|(name, decl)| Some((*name, self.name_span(decl)?)));
        spans.filter(|(_, span)| span.file == file).collect()
    }

    /// Where the name `decl` binds is written.
    fn name_span(&self, decl: &DeclRef) -> Option<Span> {
        let parsed = self.db.get::<Parsed>(&decl.file)?;
        let hir = &parsed.hir;
        Some(self.source_span(hir.pats[hir.decls[decl.decl.id].pat].origin))
    }
}
