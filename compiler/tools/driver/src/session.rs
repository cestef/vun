//! One compilation: its sources, origins and diagnostics.

use std::path::Path;

use overt_build::manifest::{self, Manifest};
use overt_diag::{DiagCtxt, DiagCx, DiagSink, Diagnostic, Msg, errors};
use overt_hir::lower;
use overt_hir::resolve::{self, Resolution};
use overt_hir::tree::Hir;
use overt_intern::Interner;
use overt_origin::OriginTable;
use overt_span::{FileId, SourceFiles, SourceMap};
use overt_syntax::cst::Cst;
use overt_syntax::tokens::Tokens;
use overt_syntax::{lexer, literal, parser};

use crate::formats::{self, Format};

pub struct Session {
    pub sources: SourceFiles,
    pub interner: Interner,
    pub origins: OriginTable,
    pub diag: DiagCtxt,
    /// How diagnostics are shown.
    pub format: &'static Format,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            sources: SourceFiles::default(),
            interner: Interner::default(),
            origins: OriginTable::default(),
            diag: DiagCtxt::default(),
            format: formats::default(),
        }
    }
}

impl Session {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads a source file; a read failure becomes a diagnostic.
    pub fn load(&mut self, path: &Path) -> Option<FileId> {
        match std::fs::read_to_string(path) {
            Ok(text) => Some(self.sources.add(path.display().to_string(), text)),
            Err(e) => {
                let msg = Msg::new()
                    .text("cannot read ")
                    .code(path.display().to_string())
                    .text(": ")
                    .text(e.to_string());
                self.diag.error(Diagnostic::error(errors::IO_UNREADABLE_FILE, msg));
                None
            }
        }
    }

    #[must_use]
    pub fn text(&self, file: FileId) -> &str {
        &self.sources.file(file).text
    }

    pub fn lex(&mut self, file: FileId) -> Tokens {
        let text = &self.sources.file(file).text;
        lexer::lex(file, text, &mut DiagCx { diag: &mut self.diag, origins: &mut self.origins })
    }

    pub fn parse(&mut self, tokens: &Tokens) -> Cst {
        let text = &self.sources.file(tokens.file()).text;
        parser::parse(tokens, text, &mut DiagCx { diag: &mut self.diag, origins: &mut self.origins })
    }

    /// Lowers a parsed file to HIR and resolves its names, reporting what needs no types.
    pub fn lower(&mut self, tokens: &Tokens, cst: &Cst) -> (Hir, Resolution) {
        let text = &self.sources.file(tokens.file()).text;
        let source = lower::Source { cst, tokens, text };
        let hir = lower::lower(source, &mut self.interner, &mut self.origins);
        let resolution = resolve::resolve(
            &hir,
            &self.interner,
            &mut DiagCx { diag: &mut self.diag, origins: &mut self.origins },
        );
        (hir, resolution)
    }

    /// Reads the manifest of the `build.ov` loaded as `file` (spec 12, section 2).
    pub fn manifest(&mut self, file: FileId) -> Option<Manifest> {
        let tokens = self.lex(file);
        let cst = self.parse(&tokens);
        let text = &self.sources.file(file).text;
        let decls = literal::declarations(&cst, &tokens, text);
        manifest::read(&decls, file, &mut DiagCx { diag: &mut self.diag, origins: &mut self.origins })
    }

    /// Emits every diagnostic in deterministic order.
    pub fn emit(&self, sink: &mut dyn DiagSink) {
        for d in self.diag.sorted() {
            sink.emit(d, &self.origins, &self.sources);
        }
    }

    #[must_use]
    pub fn failed(&self) -> bool {
        self.diag.error_count() > 0
    }
}
