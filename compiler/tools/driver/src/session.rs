//! One compilation: its sources, origins and diagnostics.

use std::path::{Path, PathBuf};

use overt_build::manifest::{self, Manifest};
use overt_build::store::Store;
use overt_diag::{DiagCtxt, DiagCx, DiagSink, Diagnostic, Msg, errors};
use overt_hir::lower;
use overt_hir::resolve::{self, Resolution};
use overt_hir::tree::Hir;
use overt_intern::Interner;
use overt_origin::OriginTable;
use overt_span::{FileId, SourceFiles, SourceMap};
use overt_syntax::cst::Cst;
use overt_syntax::literal::Decl;
use overt_syntax::tokens::Tokens;
use overt_syntax::{lexer, literal, parser};

use crate::formats::{self, Format};
use crate::packages::{FileScope, LIBRARY, Packages};

pub struct Session {
    pub sources: SourceFiles,
    pub interner: Interner,
    pub origins: OriginTable,
    pub diag: DiagCtxt,
    /// How diagnostics are shown.
    pub format: &'static Format,
    pub packages: Packages,
    /// Where default dependencies are found.
    pub library: PathBuf,
    /// The store of fetched packages, the user's unless set.
    pub store: Store,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            sources: SourceFiles::default(),
            interner: Interner::default(),
            origins: OriginTable::default(),
            diag: DiagCtxt::default(),
            format: formats::default(),
            packages: Packages::default(),
            library: PathBuf::from(LIBRARY),
            store: Store::user(),
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
        let text = self.read(path)?;
        Some(self.sources.add(path.display().to_string(), text))
    }

    /// Reads a file's text; a read failure becomes a diagnostic.
    pub fn read(&mut self, path: &Path) -> Option<String> {
        std::fs::read_to_string(path)
            .map_err(|e| {
                let msg = Msg::new().text("cannot read ").code(path.display().to_string());
                self.diag.error(Diagnostic::error(errors::IO_UNREADABLE_FILE, msg).caused_by(&e));
            })
            .ok()
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

    pub fn lower(&mut self, tokens: &Tokens, cst: &Cst) -> Hir {
        let text = &self.sources.file(tokens.file()).text;
        lower::lower(lower::Source { cst, tokens, text }, &mut self.interner, &mut self.origins)
    }

    /// Resolves the names of `file` within it and against its package, reporting what needs no
    /// types.
    pub fn resolve(&mut self, file: FileId, hir: &Hir) -> Resolution {
        let mut cx = DiagCx { diag: &mut self.diag, origins: &mut self.origins };
        let resolution = resolve::resolve(hir, &self.interner, &mut cx);
        let package = self.package_of(file);
        self.scope(package);
        let spreads = self.spreads(package, hir, &resolution);
        self.check_children(file, hir);
        let outside = FileScope { package: &self.packages.scopes[&package], spreads: &spreads };
        let mut cx = DiagCx { diag: &mut self.diag, origins: &mut self.origins };
        resolution.check_outer(hir, &outside, &self.interner, &mut cx);
        resolution
    }

    /// Reads the manifest of the `build.ov` loaded as `file` (spec 12, section 2).
    pub fn manifest(&mut self, file: FileId) -> Option<Manifest> {
        self.declarations(file, |decls, cx| manifest::read(decls, file, cx))
    }

    /// Runs `read` over the literal declarations of `file`.
    pub fn declarations<R>(
        &mut self,
        file: FileId,
        read: impl FnOnce(&[Decl<'_>], &mut DiagCx<'_>) -> R,
    ) -> R {
        let tokens = self.lex(file);
        let cst = self.parse(&tokens);
        let text = &self.sources.file(file).text;
        let decls = literal::declarations(&cst, &tokens, text);
        read(&decls, &mut DiagCx { diag: &mut self.diag, origins: &mut self.origins })
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
