//! What an editor asks of the files it holds (spec 13, section 11): what checking one reports,
//! the expression at a position with its type, value, layout and the documentation of what it
//! names, where a name at a position is declared and named, renaming it, the signature of a call
//! being written, the types and layouts code leaves unwritten, the names that may be written, the
//! file's outline, folds, highlights and formatted text, each over the queries `vun check`
//! computes.

mod complete;
mod folds;
mod format;
mod highlight;
mod hints;
mod hover;
mod layout;
mod outline;
mod references;
mod rename;
mod run;
mod signature;

pub use complete::Completion;
pub use folds::{Fold, FoldKind};
pub use highlight::{Class, Highlight, Modifier, Modifiers};
pub use hints::TypeHint;
pub use hover::Hovered;
pub use layout::Laid;
pub use outline::{Symbol, SymbolKind};
pub use references::Occurrences;
pub use rename::{Renamed, Unrenamed};
pub use signature::Signature;

use std::path::PathBuf;

use vun_diag::DiagCtxt;
use vun_hir::db::{Package, Parsed};
use vun_hir::tree::{ExprKind, Name};

use vun_origin::Resolve;
use vun_sema::decl::TypedDecl;
use vun_sema::site::{Declaration, NameSite};
use vun_sema::typeck::{Known, Typed};
use vun_sema_ty::call::Instantiation;
use vun_sema_ty::item::{DeclRef, ExprRef, Site};
use vun_span::{FileId, Span};

use crate::Session;

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
    pub(crate) fn expr_at(&mut self, file: FileId, offset: u32) -> Option<ExprRef> {
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

    /// The function literal of `file` holding the byte `offset`: the innermost one, the literal
    /// itself when `offset` is on its own text and no inner function holds it.
    pub fn function_at(&mut self, file: FileId, offset: u32) -> Option<ExprRef> {
        let expr = self.expr_at(file, offset)?;
        let hir = &self.db.get::<Parsed>(&expr.file)?.hir;
        let function = match hir.exprs[expr.expr.id].kind {
            ExprKind::Fn { .. } => expr.expr.id,
            _ => hir.enclosing_function(expr.expr.id)?,
        };
        Some(ExprRef::new(expr.file.clone(), hir, function))
    }

    /// The type typing found for `expr`: in the body of the function holding it, or else in the
    /// value of the member declaration holding it.
    pub(crate) fn known(&self, expr: &ExprRef) -> Option<Known> {
        let parsed = self.db.get::<Parsed>(&expr.file)?;
        let hir = &parsed.hir;
        let typed = if let Some(function) = hir.enclosing_function(expr.expr.id) {
            self.db.get::<Typed>(&Instantiation::plain(ExprRef::new(expr.file.clone(), hir, function)))?
        } else {
            let decl = DeclRef::new(expr.file.clone(), hir, hir.enclosing_member(expr.expr.id)?);
            self.db.get::<TypedDecl>(&(decl, None, Vec::new()))?
        };
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
            Declaration::Site(Site::Variant { decl, name }) => {
                let parsed = self.db.get::<Parsed>(&decl.file)?;
                let variants = parsed.hir.variants_of(decl.decl.id);
                let variant = variants.iter().find(|v| v.name == Name::Plain(name))?;
                Some(Target::At(self.source_span(variant.origin)))
            }
            Declaration::Site(Site::Namespace(ns)) => {
                let package = self.db.get::<Package>(&ns.package);
                package.file_of(&ns.path).map(|f| Target::File(PathBuf::from(package.display(f))))
            }
        }
    }
}
