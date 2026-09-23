//! One compilation: its sources, origins and diagnostics.

use std::path::Path;

use overt_diag::{DiagCtxt, DiagCx, DiagSink, Diagnostic, Msg, errors};
use overt_origin::OriginTable;
use overt_span::{FileId, SourceFiles, SourceMap};
use overt_syntax::lexer;
use overt_syntax::tokens::Tokens;

#[derive(Default)]
pub struct Session {
    pub sources: SourceFiles,
    pub origins: OriginTable,
    pub diag: DiagCtxt,
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
