//! The nodes lints look at in a file: its declarations, the fields and variants of its types, and
//! the calls in their values, each with the lints allowed around it.

use vun_hir::tree::{DeclId, Entry, ExprId, ExprKind, FieldDef, Hir, Member, Name, NsId, PatKind, Variant};
use vun_hir::visit::{self, Visitor};
use vun_intern::Symbol;
use vun_origin::OriginId;
use vun_primitives::Primitive;
use vun_primitives::lint::Lint;
use vun_sema_ty::item::DeclRef;
use vun_sema_ty::{TyKind, Value};

use super::literal::Literal;
use super::tables::Binds;
use crate::callable::Callable;
use crate::decl::DeclValue;
use crate::env::Env;

/// A node of a kind lints look at.
pub(super) enum Kind {
    /// A declaration: its name and what it binds.
    Decl { name: Symbol, binds: Binds },
    /// A call, by the value of what it calls.
    Call { callee: Value },
}

/// A node lints look at: its kind, where it is written, and the lints `#[lint.allow]` silences
/// around it.
pub(super) struct Node {
    pub(super) kind: Kind,
    pub(super) origin: OriginId,
    pub(super) allowed: Vec<Value>,
}

impl Node {
    /// The nodes of the file `env` reads.
    pub(super) fn of(env: &Env<'_, '_>) -> Vec<Self> {
        let mut walk = Walk { env, nodes: Vec::new(), allowed: Vec::new() };
        if let Some(root) = env.parsed.hir.root {
            walk.namespace(root);
        }
        walk.nodes
    }
}

struct Walk<'e, 'a, 'db> {
    env: &'e Env<'a, 'db>,
    nodes: Vec<Node>,
    /// The lints allowed by the declarations around the one walked.
    allowed: Vec<Value>,
}

impl Walk<'_, '_, '_> {
    fn hir(&self) -> &Hir {
        &self.env.parsed.hir
    }

    fn namespace(&mut self, ns: NsId) {
        let members: Vec<_> =
            self.env.parsed.hir.namespaces[ns].members.iter().filter_map(Entry::written).collect();
        for member in members {
            match member {
                Member::Decl(d) => self.decl(*d),
                Member::Field(f) => self.field(f),
                Member::Spread(_) => {}
            }
        }
    }

    /// `f` run with the lints `attrs` allows added to those allowed around.
    fn allowing(&mut self, attrs: &[ExprId], f: impl FnOnce(&mut Self)) {
        let outer = self.allowed.len();
        let allows = self.env.attributes(attrs, Primitive::Lint(Lint::allow));
        self.allowed.extend(allows.into_iter().filter_map(|args| args.into_iter().next()));
        f(self);
        self.allowed.truncate(outer);
    }

    fn push(&mut self, kind: Kind, origin: OriginId) {
        self.nodes.push(Node { kind, origin, allowed: self.allowed.clone() });
    }

    fn decl(&mut self, id: DeclId) {
        let d = &self.env.parsed.hir.decls[id];
        let (pat, value) = (d.pat, d.value);
        self.allowing(&d.attrs, |walk| {
            if let PatKind::Bind(b) = walk.hir().pats[pat].kind {
                let binder = &walk.hir().binders[b];
                let (name, origin) = (binder.name, binder.origin);
                if let Some(binds) = walk.binds(id) {
                    walk.push(Kind::Decl { name, binds }, origin);
                }
            }
            if let Some(value) = value {
                walk.value(value);
            }
        });
    }

    /// A field: one holding a type binds a type.
    fn field(&mut self, f: &FieldDef) {
        self.allowing(&f.attrs, |walk| {
            if let Name::Plain(name) = f.name {
                let binds = if walk.types(f.ty) { Binds::Type } else { Binds::Field };
                walk.push(Kind::Decl { name, binds }, f.origin);
            }
        });
    }

    fn variant(&mut self, v: &Variant) {
        if let Name::Plain(name) = v.name {
            self.push(Kind::Decl { name, binds: Binds::Variant }, v.origin);
        }
    }

    /// A declaration's value: the members of a type it writes, or the calls it makes.
    fn value(&mut self, expr: ExprId) {
        match &self.env.parsed.hir.exprs[expr].kind {
            ExprKind::Struct(ns) => self.namespace(*ns),
            ExprKind::Enum { variants, ns } => {
                for v in variants.iter().filter_map(Entry::written) {
                    self.variant(v);
                }
                self.namespace(*ns);
            }
            ExprKind::Attributed { inner, .. } => self.value(*inner),
            _ => {
                let mut calls = Calls { env: self.env, found: Vec::new() };
                calls.expr(self.hir(), expr);
                for (callee, origin) in calls.found {
                    self.push(Kind::Call { callee }, origin);
                }
            }
        }
    }

    /// What the declaration `id` binds (spec 10, section 5): a type, an interface or a function
    /// giving one, unless what it gives is a namespace, a struct of declarations alone; any
    /// other value. One annotated `type` or `lang.Interface` binds a type, as `()` does there; a
    /// primitive, `= _`, binds what the annotation of its declaration says. `None` for one whose
    /// value is not known, which lints do not guess.
    fn binds(&self, id: DeclId) -> Option<Binds> {
        let hir = self.hir();
        if hir.decls[id].ty.is_some_and(|t| self.types(t)) {
            return Some(Binds::Type);
        }
        let cx = self.env.cx;
        let decl = DeclRef::new(self.env.file.clone(), hir, id);
        let gives = match cx.get::<DeclValue>(&decl).ok()? {
            Value::Type(ty) => match &*ty {
                TyKind::Nominal(n) => !Literal::of(cx, &n.literal).is_some_and(|l| l.namespace()),
                _ => true,
            },
            Value::Interface(_) => true,
            Value::Fn(f) => Literal::of(cx, &f.literal).is_some_and(|l| l.gives_type()),
            Value::Primitive(p) => {
                Callable::Primitive(p).written(self.env).is_some_and(|w| Literal::from(w).gives_type())
            }
            _ => false,
        };
        Some(if gives { Binds::Type } else { Binds::Value })
    }

    /// Whether the annotation `expr` denotes `type` or `lang.Interface`: what binds a type.
    fn types(&self, expr: ExprId) -> bool {
        self.env.ty(expr).is_ok_and(|t| t.holds_types())
    }
}

/// The calls in an expression whose callee is known at compile time, with where each is written.
struct Calls<'e, 'a, 'db> {
    env: &'e Env<'a, 'db>,
    found: Vec<(Value, OriginId)>,
}

impl Visitor for Calls<'_, '_, '_> {
    fn expr(&mut self, hir: &Hir, expr: ExprId) {
        if let ExprKind::Call { callee, .. } = &hir.exprs[expr].kind
            && let Ok(value) = self.env.eval(*callee)
        {
            self.found.push((value, hir.exprs[expr].origin));
        }
        visit::walk_expr(self, hir, expr);
    }

    /// A namespace inside a value is a type's, whose members are declarations of their own.
    fn namespace(&mut self, _: &Hir, _: NsId) {}
}
