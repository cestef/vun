//! What an editor asks of the files it holds (spec 13, section 11): what checking one reports,
//! the expression at a position with its type and the documentation of what it names, where a
//! name at a position is declared and named, renaming it, the signature of a call being written,
//! the types code leaves unwritten, the names that may be written, the file's outline, folds,
//! highlights and formatted text, each over the queries `vun check` computes.

mod complete;
mod folds;
mod format;
mod highlight;
mod hints;
mod outline;
mod references;
mod rename;
mod signature;

pub use complete::Completion;
pub use folds::{Fold, FoldKind};
pub use highlight::{Class, Highlight, Modifier, Modifiers};
pub use hints::TypeHint;
pub use outline::{Symbol, SymbolKind};
pub use references::Occurrences;
pub use rename::{Renamed, Unrenamed};
pub use signature::Signature;

use std::path::PathBuf;

use vun_diag::DiagCtxt;
use vun_hir::db::{Package, Parsed};
use vun_hir::tree::ExprId;
use vun_origin::Resolve;
use vun_sema::site::{Declaration, NameSite};
use vun_sema::typeck::{Known, Typed};
use vun_sema_ty::call::Instantiation;
use vun_sema_ty::item::{ExprRef, Site};
use vun_span::{FileId, Span};

use crate::Session;

/// What hovering over a position shows.
pub struct Hovered {
    /// The expression hovered over.
    pub span: Span,
    /// Its type and stage, where typing found them.
    pub known: Option<Known>,
    /// The doc comment of the declaration it names, line by line.
    pub doc: Vec<String>,
}

/// Where a declaration is.
pub enum Target {
    At(Span),
    /// A namespace: its file, from its start.
    File(PathBuf),
}

impl Session {
    /// `file` parsed, what that reported left to checking, which reports it once.
    fn syntax(&mut self, file: FileId) -> std::sync::Arc<vun_hir::db::ParsedFile> {
        let key = self.file_ref(file);
        self.db.get::<Parsed>(&key).expect("the file's text is set")
    }

    /// Checks `file` as `vun check` does, and takes out what that reports.
    pub fn reported(&mut self, file: FileId) -> DiagCtxt {
        self.check(file);
        std::mem::take(&mut self.diag)
    }

    /// The innermost expression of `file` holding the byte `offset`.
    fn expr_at(&mut self, file: FileId, offset: u32) -> Option<ExprRef> {
        let key = self.file_ref(file);
        let parsed = self.db.get::<Parsed>(&key)?;
        let holding = parsed.hir.exprs.iter().filter_map(|(id, expr)| {
            let span = self.source_span(expr.origin);
            let holds = span.file == file && span.range.lo <= offset && offset < span.range.hi;
            holds.then_some((span.range.hi - span.range.lo, id))
        });
        let (_, id) = holding.min_by_key(|&(len, _)| len)?;
        Some(ExprRef::new(key, &parsed.hir, id))
    }

    /// What hovering at `offset` of `file` shows.
    pub fn hovered(&mut self, file: FileId, offset: u32) -> Option<Hovered> {
        let expr = self.expr_at(file, offset)?;
        let span = self.source_span(expr.expr.origin);
        let known = self.known(&expr);
        let doc = match self.db.get::<NameSite>(&expr) {
            Some(Declaration::Site(Site::Decl(decl))) => self
                .db
                .get::<Parsed>(&decl.file)
                .map(|p| p.hir.decls[decl.decl.id].doc.clone())
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        Some(Hovered { span, known, doc })
    }

    /// The type typing found for `expr`, in the body of the function holding it.
    fn known(&self, expr: &ExprRef) -> Option<Known> {
        let parsed = self.db.get::<Parsed>(&expr.file)?;
        let function: ExprId = parsed.hir.enclosing_function(expr.expr.id)?;
        let function = ExprRef::new(expr.file.clone(), &parsed.hir, function);
        let typed = self.db.get::<Typed>(&Instantiation::plain(function))?;
        typed.types.get(&expr.expr.id).copied()
    }

    /// Where the name at `offset` of `file` is declared.
    pub fn declared(&mut self, file: FileId, offset: u32) -> Option<Target> {
        let expr = self.expr_at(file, offset)?;
        match self.db.get::<NameSite>(&expr)? {
            Declaration::Local(origin) => Some(Target::At(self.source_span(origin))),
            Declaration::Site(Site::Decl(decl)) => {
                // The name it declares, not the whole declaration.
                let parsed = self.db.get::<Parsed>(&decl.file)?;
                let hir = &parsed.hir;
                Some(Target::At(self.source_span(hir.pats[hir.decls[decl.decl.id].pat].origin)))
            }
            Declaration::Site(Site::Namespace(ns)) => {
                let package = self.db.get::<Package>(&ns.package);
                package.file_of(&ns.path).map(|f| Target::File(PathBuf::from(package.display(f))))
            }
        }
    }
}
