//! What hovering over a position shows: the name there, its type and stage, the value compiling
//! computes for the declaration it names, the layout of the type it is or has, and the doc
//! comment of that declaration. A declaration's own name shows the declaration; any other
//! position, the innermost expression holding it.

use vun_hir::db::Parsed;
use vun_hir::tree::{ExprKind, Name, PatKind};
use vun_intern::Symbol;
use vun_origin::Resolve;
use vun_sema::decl::{DeclType, DeclValue};
use vun_sema::site::{Declaration, NameSite};
use vun_sema::typed::{Known, Stage};
use vun_sema::value::ExprValue;
use vun_sema_ty::item::{DeclRef, ExprRef, Site};
use vun_sema_ty::{Nominal, TyKind, Value};
use vun_span::{FileId, Span};

use super::Laid;
use crate::Session;

pub struct Hovered {
    /// The name or the expression hovered over.
    pub span: Span,
    /// The name it is, if it is one.
    pub name: Option<Symbol>,
    /// Its type and stage, where typing found them.
    pub known: Option<Known>,
    /// Its value when compiling computes one other than a literal it writes, which its name and
    /// type say: the value of the declaration it names, or else its own.
    pub value: Option<Value>,
    /// The layout of the type it is, or else of its type.
    pub laid: Option<Laid>,
    /// The doc comment of the declaration it names, line by line.
    pub doc: Vec<String>,
}

/// What a position is: a name declared or an expression.
struct Found {
    span: Span,
    /// The name it is, if it is one.
    name: Option<Symbol>,
    known: Option<Known>,
    /// The declaration it is or names.
    decl: Option<DeclRef>,
    /// The expression whose value it has: itself, or the value its declaration writes.
    expr: Option<ExprRef>,
}

impl Session {
    /// What hovering at `offset` of `file` shows.
    pub fn hovered(&mut self, file: FileId, offset: u32) -> Option<Hovered> {
        let found = match self.declared_at(file, offset) {
            Some(found) => found,
            None => self.expr_hovered(file, offset)?,
        };
        let Found { span, name, known, decl, expr } = found;
        let (mut value, mut doc) = (None, Vec::new());
        if let Some(decl) = decl {
            let parsed = self.db.get::<Parsed>(&decl.file);
            doc = parsed.map(|p| p.hir.decls[decl.decl.id].doc.clone()).unwrap_or_default();
            value = self.db.get::<DeclValue>(&decl).ok().filter(|v| !Self::literal(v));
        }
        if value.is_none() && known.is_some_and(|k| k.stage == Stage::Compile) {
            let computed = expr.and_then(|e| self.db.get::<ExprValue>(&e).ok());
            value = computed.filter(|v| !Self::literal(v));
        }
        let ty = match &value {
            Some(Value::Type(ty)) => Some(*ty),
            _ => known.map(|k| k.ty),
        };
        let laid = ty.and_then(|ty| self.laid(&ty));
        Some(Hovered { span, name, known, value, laid, doc })
    }

    /// What the innermost expression of `file` holding `offset` is.
    fn expr_hovered(&mut self, file: FileId, offset: u32) -> Option<Found> {
        let expr = self.expr_at(file, offset)?;
        let name = match self.db.get::<Parsed>(&expr.file)?.hir.exprs[expr.expr.id].kind {
            ExprKind::Name(name) | ExprKind::Member { name: Name::Plain(name), .. } => Some(name),
            _ => None,
        };
        let decl = match self.db.get::<NameSite>(&expr) {
            Some(Declaration::Site(Site::Decl(decl))) => Some(decl),
            _ => None,
        };
        let known = self.known(&expr).or_else(|| self.annotated(&expr));
        let span = self.source_span(expr.expr.origin);
        Some(Found { span, name, known, decl, expr: Some(expr) })
    }

    /// The type and stage of a type written where typing records none, as an annotation: `type`,
    /// at compile time, when it evaluates to a type.
    fn annotated(&self, expr: &ExprRef) -> Option<Known> {
        let Value::Type(_) = self.db.get::<ExprValue>(expr).ok()? else { return None };
        Some(Known { ty: TyKind::of_types(), stage: Stage::Compile })
    }

    /// What the declaration of `file` whose name holds `offset` is.
    fn declared_at(&mut self, file: FileId, offset: u32) -> Option<Found> {
        let key = self.file_ref(file);
        let parsed = self.db.get::<Parsed>(&key)?;
        let hir = &parsed.hir;
        let (decl, span, name) = hir.decls.iter().find_map(|(id, decl)| {
            let PatKind::Bind(binder) = hir.pats[decl.pat].kind else { return None };
            let span = self.source_span(hir.binders[binder].origin);
            let holds = span.file == file && span.range.lo <= offset && offset < span.range.hi;
            holds.then(|| (DeclRef::new(key.clone(), hir, id), span, hir.binders[binder].name))
        })?;
        let expr = hir.decls[decl.decl.id].value.map(|v| ExprRef::new(key.clone(), hir, v));
        Some(Found { span, name: Some(name), known: self.decl_known(&decl), decl: Some(decl), expr })
    }

    /// The type and stage of the value `decl` declares: as its declaration when it is a member of
    /// a namespace, or as typing found its value in the function holding it.
    pub(crate) fn decl_known(&self, decl: &DeclRef) -> Option<Known> {
        let parsed = self.db.get::<Parsed>(&decl.file)?;
        let hir = &parsed.hir;
        let declared = &hir.decls[decl.decl.id];
        let PatKind::Bind(binder) = hir.pats[declared.pat].kind else { return None };
        match hir.member_decl(binder) {
            Some(_) => self.db.get::<DeclType>(decl).ok(),
            None => self.known(&ExprRef::new(decl.file.clone(), hir, declared.value?)),
        }
    }

    /// Whether `value` is a function or a type literal as written, not computed by a call.
    fn literal(value: &Value) -> bool {
        match value {
            Value::Fn(_) | Value::Closure { .. } => true,
            Value::Type(ty) => matches!(&**ty, TyKind::Nominal(Nominal { instance: None, .. })),
            _ => false,
        }
    }
}
