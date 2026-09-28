//! One compilation: its sources, origins and diagnostics.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use vun_build::manifest::Manifest;
use vun_build::store::Store;
use vun_command::Diagnostics;
use vun_diag::{DiagCtxt, DiagCx, Diagnostic, ErrorGuaranteed};
use vun_fetch::Schemes;
use vun_hir::db::{FileNames, FileRef, FileText, Parsed, ParsedFile};
use vun_hir::lower;
use vun_hir::tree::{ExprKind, Hir};
use vun_intern::Symbol;
use vun_interp::Interpreter;
use vun_lower::{Lowered, Unlowered};
use vun_mir::{Body, Mir, Program, Programs, Roots, Unbuilt};
use vun_origin::OriginTable;
use vun_query::Db;
use vun_sema::check::Checked;
use vun_sema::decl::DeclValue;
use vun_sema::eval::Evaluators;
use vun_sema::eval::Instantiation;
use vun_sema::item::{DeclRef, ExprRef};
use vun_sema::layout::Framers;
use vun_sema::target::Choice;
use vun_sema::ty::{Ty, Value};
use vun_span::{FileId, SourceFiles, SourceMap};
use vun_syntax::cst::Cst;
use vun_syntax::literal::Decl;
use vun_syntax::tokens::Tokens;
use vun_target::TargetSpec;
use vun_term::Shell;
use vun_trace::tracing;

use crate::formats::Format;
use crate::packages::{LIBRARY, Packages};
use crate::registry::Codegen;
use crate::{Persistence, Registry};

pub struct Session {
    /// The queries of the compilation; other sessions may share them (`Session::sharing`).
    pub db: Arc<Db>,
    /// The process's files and origins, which every compilation in it shares.
    pub sources: &'static SourceFiles,
    pub origins: &'static OriginTable,
    pub diag: DiagCtxt,
    /// How diagnostics are shown.
    pub format: &'static Format,
    pub packages: Packages,
    /// Where default dependencies are found.
    pub library: PathBuf,
    /// The store of fetched packages, the user's unless set.
    pub store: Store,
    /// The URL schemes packages are downloaded through: `file://` unless the program installs
    /// more, as `vun` does with `vun-http`.
    pub schemes: Schemes,
    /// How the target is chosen: the running machine's unless `--target` names one.
    pub choice: Choice,
    /// The target, once chosen, or the report that none is.
    pub target: Option<Result<TargetSpec, ErrorGuaranteed>>,
    /// The backends and linkers compiled into this toolchain.
    pub registry: Registry,
    /// Which backend builds: the target's preferences for the profile unless one is named.
    pub codegen: Codegen,
    /// Whether the queries are saved between runs; off unless the program turns it on.
    pub persistence: Persistence,
    /// The machine's toolchain file, the user's unless set (`Toolchain::path`).
    pub toolchain: Option<PathBuf>,
    /// The command programs built for the target run through, from the toolchain file; empty
    /// where they run as they are.
    pub runner: Vec<Symbol>,
    /// The memory map of the program built, from its package's plan; empty for a program an
    /// operating system loads.
    pub memory: Vec<vun_link::MemoryRegion>,
    /// What `vun util lsp` runs: the language server the program installs, as `vun` does.
    pub language_server: Option<fn(&mut Session) -> std::io::Result<vun_command::Outcome>>,
    /// How the program running the session writes its `check` command, for messages that
    /// suggest it; `None` where no command line runs the session.
    pub check: Option<String>,
    /// The errors already shown and taken out of `diag`.
    shown_errors: usize,
}

impl Default for Session {
    fn default() -> Self {
        Self::sharing(&Arc::new(Db::new()))
    }
}

impl Session {
    /// A session over the queries `db`, which other sessions may compute in at the same time:
    /// what each computes, every other reuses. Its diagnostics and packages are its own; all
    /// compile for one target, which `Shared` ensures.
    #[must_use]
    pub fn sharing(db: &Arc<Db>) -> Self {
        let db = Arc::clone(db);
        db.resource::<Evaluators>().install(Arc::new(Interpreter));
        db.resource::<Framers>().install(Arc::new(vun_lower::Framing));
        Self::register(&db);
        Self {
            sources: SourceFiles::global(),
            origins: OriginTable::global(),
            db,
            diag: DiagCtxt::default(),
            format: Format::DEFAULT,
            packages: Packages::default(),
            library: PathBuf::from(LIBRARY),
            store: Store::user(),
            schemes: Schemes::default(),
            choice: Choice::host(),
            target: None,
            registry: Registry::with_defaults(),
            codegen: Codegen::default(),
            persistence: Persistence::Off,
            toolchain: vun_build::toolchain::Toolchain::path(),
            runner: Vec::new(),
            memory: Vec::new(),
            language_server: None,
            check: None,
            shown_errors: 0,
        }
    }

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads a source file; a read failure becomes a diagnostic.
    pub fn load(&mut self, path: &Path) -> Option<FileId> {
        let _timer = crate::stages::LOAD.time();
        tracing::info!(path = %path.display(), "loading");
        let text = self.read(path)?;
        Some(self.open(path, text))
    }

    /// Reads `text` as the file at `path`, saved or not: a new version of it, which the queries
    /// read from now on, and in which what they computed from an earlier one is shown.
    pub fn open(&mut self, path: &Path, text: String) -> FileId {
        self.restore(path);
        let key = self.file_key(path);
        self.sources.add_as(key, path.display().to_string(), text)
    }

    /// Reads a file's text; a read failure becomes a diagnostic.
    pub fn read(&mut self, path: &Path) -> Option<String> {
        std::fs::read_to_string(path)
            .map_err(|e| {
                self.diag.error(Diagnostic::unreadable(path, &e));
            })
            .ok()
    }

    #[must_use]
    pub fn text(&self, file: FileId) -> &str {
        &self.sources.file(file).text
    }

    pub fn lex(&mut self, file: FileId) -> Tokens {
        let text = &self.sources.file(file).text;
        Tokens::lex(file, text, &mut DiagCx { diag: &mut self.diag, origins: self.origins })
    }

    pub fn parse(&mut self, tokens: &Tokens) -> Cst {
        let text = &self.sources.file(tokens.file()).text;
        Cst::parse(tokens, text, &mut DiagCx { diag: &mut self.diag, origins: self.origins })
    }

    pub fn lower(&mut self, tokens: &Tokens, cst: &Cst) -> Hir {
        let text = &self.sources.file(tokens.file()).text;
        Hir::lower(lower::Source { cst, tokens, text }, self.origins)
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

    /// The function literals of `file`, in source order.
    pub fn functions(&mut self, file: FileId) -> Vec<ExprRef> {
        let key = self.file_ref(file);
        let parsed = self.parsed(file);
        let literals = parsed.hir.exprs.iter().filter(|(_, e)| matches!(e.kind, ExprKind::Fn { .. }));
        literals.map(|(id, _)| ExprRef::new(key.clone(), &parsed.hir, id)).collect()
    }

    /// The types `file` binds at its top level, by name, in source order.
    pub fn types(&mut self, file: FileId) -> Vec<(Symbol, Ty)> {
        let key = self.file_ref(file);
        let parsed = self.parsed(file);
        let hir = &parsed.hir;
        let Some(root) = hir.root else { return Vec::new() };
        let mut types = Vec::new();
        for decl in hir.member_decls(root) {
            let Ok(Value::Type(ty)) = self.db.get::<DeclValue>(&DeclRef::new(key.clone(), hir, decl)) else {
                continue;
            };
            let mut binders = Vec::new();
            hir.binders(hir.decls[decl].pat, &mut binders);
            types.extend(binders.first().map(|&b| (hir.binders[b].name, ty)));
        }
        types
    }

    /// The program whose root is `file`: its exports and the functions they reach.
    pub fn program(&mut self, file: FileId) -> Arc<Program> {
        let key = self.file_ref(file);
        self.db.get::<Programs>(&(key, Roots::Exports))
    }

    /// The LIR of the program whose root is `file`, or why it does not lower.
    pub fn lir(&mut self, file: FileId) -> Arc<Result<vun_lir::Unit, Unlowered>> {
        let key = self.file_ref(file);
        self.db.get::<Lowered>(&key)
    }

    /// The MIR of each function literal of `file`, in source order.
    pub fn bodies(&mut self, file: FileId) -> Vec<(ExprRef, Result<Arc<Body>, Unbuilt>)> {
        let functions = self.functions(file);
        functions.into_iter().map(|f| (f.clone(), self.db.get::<Mir>(&Instantiation::plain(f)))).collect()
    }

    /// How the queries name `file`, with the inputs of its package graph and its text set.
    pub fn file_ref(&mut self, file: FileId) -> FileRef {
        let path = PathBuf::from(&self.sources.file(file).path);
        let package = self.package_of(file);
        self.publish(package);
        let key = self.packages.list[&package].file(&path);
        // The file as loaded, which its package's files do not include when it is loose.
        self.db.set::<FileText>(&key, Some(Arc::from(self.text(file))));
        // Reported once when none is; what needs it then stays unknown.
        let _ = self.target_for(&key);
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

    /// Reads the manifest of the `build.vun` loaded as `file` (spec 12, section 2).
    pub fn manifest(&mut self, file: FileId) -> Option<Manifest> {
        self.declarations(file, |decls, cx| Manifest::read(decls, file, cx))
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
        let decls = cst.declarations(&tokens, text);
        read(&decls, &mut DiagCx { diag: &mut self.diag, origins: self.origins })
    }

    /// Emits every diagnostic in deterministic order.
    /// Shows the diagnostics collected so far on the shell's standard error, in the session's
    /// format, and takes them out; the session stays failed if one was an error.
    pub fn emit(&mut self, shell: &mut Shell<'_>) {
        let diag = std::mem::take(&mut self.diag);
        let mut sink = shell.diagnostics(self.format.sink);
        for d in diag.sorted(self) {
            sink.emit(d, self, self.sources);
        }
        self.shown_errors += diag.error_count();
    }

    #[must_use]
    pub fn failed(&self) -> bool {
        self.diag.error_count() + self.shown_errors > 0
    }
}
