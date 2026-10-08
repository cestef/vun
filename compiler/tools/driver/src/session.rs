//! One compilation: its sources, origins and diagnostics.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use vun_build::manifest::Manifest;
use vun_build::store::Store;
use vun_command::Diagnostics;
use vun_diag::{DiagCtxt, DiagCx, Diagnostic, ErrorGuaranteed};
use vun_hir::db::{FileNames, FileRef, FileText, Parsed, ParsedFile};
use vun_hir::lower;
use vun_hir::tree::{ExprKind, Hir};
use vun_intern::Symbol;
use vun_mir::Roots;
use vun_origin::OriginTable;
use vun_query::Db;
use vun_sema::decl::DeclValue;
use vun_sema::target::Choice;
use vun_sema_ty::item::{DeclRef, ExprRef};
use vun_sema_ty::{Ty, Value};
use vun_span::{FileId, SourceFiles, SourceMap};
use vun_syntax::cst::Cst;
use vun_syntax::literal::Decl;
use vun_syntax::tokens::Tokens;
use vun_target::TargetSpec;
use vun_term::Shell;
use vun_trace::tracing;

use crate::formats;
use crate::packages::{Library, Packages};
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
    pub format: Symbol,
    pub packages: Packages,
    /// Where default dependencies are found.
    pub library: PathBuf,
    /// The store of fetched packages, the user's unless set.
    pub store: Store,
    /// How the target is chosen: the running machine's unless `--target` names one.
    pub choice: Choice,
    /// The target, once chosen, or the report that none is.
    pub target: Option<Result<TargetSpec, ErrorGuaranteed>>,
    /// The backends, linkers and URL schemes of this toolchain.
    pub registry: Registry,
    /// Which backend builds: the target's preferences for the profile unless one is named.
    pub codegen: Codegen,
    /// Where the program built from a root file starts: its exports unless set, as to the
    /// members of the type running a namespace's tests.
    pub roots: Roots,
    /// Whether the queries are saved between runs; off unless the program turns it on.
    pub persistence: Persistence,
    /// The directory of the libraries' memos shared between projects, if they are shared.
    pub shared: Option<PathBuf>,
    /// Whether what the queries did with their memos is reported at exit (`Tally`).
    pub tally: bool,
    /// The machine's tools file, the user's unless set (`Tools::path`).
    pub tools: Option<PathBuf>,
    /// The entry of the machine's tools file the target is equipped with, if one names it (its
    /// runner is what programs built for the target run through: `Session::runner`).
    pub equipped: Option<vun_build::tools::TargetTools>,
    /// The memory map of the program built, from its package's plan; empty for a program an
    /// operating system loads.
    pub memory: Vec<vun_link::MemoryRegion>,
    /// Where its sections lie in `memory`, from its package's plan.
    pub sections: Vec<vun_link::SectionPlacement>,
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
        Self::provide(&db);
        Self::register(&db);
        Self {
            sources: SourceFiles::global(),
            origins: OriginTable::global(),
            db,
            diag: DiagCtxt::default(),
            format: Symbol::intern(formats::DEFAULT),
            packages: Packages::default(),
            library: Library::find().dir,
            store: Store::user(),
            choice: Choice::Host,
            target: None,
            registry: Registry::with_defaults(),
            codegen: Codegen::default(),
            roots: Roots::default(),
            persistence: Persistence::Off,
            tally: std::env::var(vun_query::Tally::VAR).is_ok_and(|v| v == "1"),
            shared: None,
            tools: vun_build::tools::Tools::path(),
            equipped: None,
            memory: Vec::new(),
            sections: Vec::new(),
            check: None,
            shown_errors: 0,
        }
    }

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A session of a program that runs for long or often, as `vun` does: its queries are saved
    /// between runs, and the memos of the libraries are shared with the other projects.
    #[must_use]
    pub fn persistent() -> Self {
        let mut session = Self::new();
        session.persistence = Persistence::On;
        session.shared = Some(session.store.memos());
        session
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
        let mut sink = shell.diagnostics(self.registry.renderer(self.format));
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
