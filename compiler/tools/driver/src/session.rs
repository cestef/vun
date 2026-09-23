//! One compilation: its sources, origins and diagnostics.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use overt_build::manifest::{self, Manifest};
use overt_build::store::Store;
use overt_diag::{DiagCtxt, DiagCx, DiagSink, Diagnostic, Msg, errors};
use overt_hir::db::{FileNames, FileRef, FileText, Parsed, ParsedFile};
use overt_hir::lower;
use overt_hir::tree::Hir;
use overt_origin::OriginTable;
use overt_query::Db;
use overt_sema::check::Checked;
use overt_span::{FileId, SourceFiles, SourceMap};
use overt_syntax::cst::Cst;
use overt_syntax::literal::Decl;
use overt_syntax::tokens::Tokens;
use overt_syntax::{lexer, literal, parser};

use crate::formats::{self, Format};
use crate::packages::{LIBRARY, Packages};

pub struct Session {
    /// The queries of the compilation, whose shared tables `sources` and `origins` are.
    pub db: Db,
    pub sources: Arc<SourceFiles>,
    pub origins: Arc<OriginTable>,
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
        let db = Db::new();
        Self {
            sources: db.resource(),
            origins: db.resource(),
            db,
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
        let key = self.file_key(path);
        Some(self.sources.add_as(&key, path.display().to_string(), text))
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
        lexer::lex(file, text, &mut DiagCx { diag: &mut self.diag, origins: &self.origins })
    }

    pub fn parse(&mut self, tokens: &Tokens) -> Cst {
        let text = &self.sources.file(tokens.file()).text;
        parser::parse(tokens, text, &mut DiagCx { diag: &mut self.diag, origins: &self.origins })
    }

    pub fn lower(&mut self, tokens: &Tokens, cst: &Cst) -> Hir {
        let text = &self.sources.file(tokens.file()).text;
        lower::lower(lower::Source { cst, tokens, text }, &self.origins)
    }

    /// The file lexed, parsed, lowered and resolved within itself, from the queries, with the
    /// inputs of its package graph set first; what those stages reported is recorded.
    ///
    /// # Panics
    /// Never: the file's text is set just before.
    pub fn parsed(&mut self, file: FileId) -> Arc<ParsedFile> {
        let key = self.file_ref(file);
        let parsed = self.db.get::<Parsed>(&key).expect("its text is set");
        self.diag.absorb(&parsed.diag);
        parsed
    }

    /// How the queries name `file`, with the inputs of its package graph and its text set.
    fn file_ref(&mut self, file: FileId) -> FileRef {
        let path = PathBuf::from(&self.sources.file(file).path);
        let package = self.package_of(file);
        let id = self.publish(package);
        let key = FileRef { package: id, path: self.packages.list[&package].relative(&path) };
        // The file as loaded, which its package's files do not include when it is loose.
        self.db.set::<FileText>(key.clone(), Some(Arc::from(self.text(file))));
        key
    }

    /// Checks the types of `file`, as far as sema does yet.
    pub fn check_types(&mut self, file: FileId) {
        let key = self.file_ref(file);
        if let Some(reports) = self.db.get::<Checked>(&key) {
            self.diag.absorb(&reports);
        }
    }

    /// Resolves the names of `file` against its package, reporting what needs no types.
    pub fn check_names(&mut self, file: FileId) {
        let key = self.file_ref(file);
        if let Some(checked) = self.db.get::<FileNames>(&key) {
            self.diag.absorb(&checked.diag);
        }
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
        read(&decls, &mut DiagCx { diag: &mut self.diag, origins: &self.origins })
    }

    /// Emits every diagnostic in deterministic order.
    pub fn emit(&self, sink: &mut dyn DiagSink) {
        for d in self.diag.sorted() {
            sink.emit(d, &self.origins, &*self.sources);
        }
    }

    #[must_use]
    pub fn failed(&self) -> bool {
        self.diag.error_count() > 0
    }
}
