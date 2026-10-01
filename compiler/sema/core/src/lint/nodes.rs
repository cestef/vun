//! The nodes lints look at in a file: its declarations, the fields and variants of its types, and
//! the calls in their values, each with the lints allowed around it.

use std::collections::HashSet;

use vun_hir::resolve::Res;
use vun_hir::tree::{
    BinderId, DeclId, Entry, ExprId, ExprKind, FieldDef, Hir, Member, Name, NsId, PatKind, Variant,
};
use vun_hir::visit::{self, Visitor};
use vun_intern::Symbol;
use vun_origin::OriginId;
use vun_primitives::Primitive;
use vun_primitives::lint::Lint;
use vun_sema_ty::Value;
use vun_sema_ty::item::ExprRef;

use super::literal::Literal;
use super::locals::{Bound, Locals};
use super::tables::Binds;
use crate::env::Env;
use crate::visible::Reach;

/// A node of a kind lints look at.
pub(super) enum Kind {
    /// A member of a namespace or a type: its name, what it binds, whether any package sees it,
    /// and whether a doc comment says what it is.
    Decl { name: Symbol, binds: Binds, public: bool, documented: bool },
    /// A local binding or a parameter: its name, whether the code names it, whether it is a
    /// parameter, whether a pattern testing the value it matches binds it, and whether it hides
    /// a constant from outside the file.
    Local { name: Symbol, used: bool, param: bool, tests: bool, hides: bool },
    /// A call, by the value of what it calls.
    Call { callee: Value },
}

/// A node lints look at: its kind, where it is written, and the lints `#[lint.allow]` silences
/// around it.
pub(super) struct Node {
    pub(super) kind: Kind,
    pub(super) origin: OriginId,
    pub(super) allowed: Vec<Value>,
    /// The declaration, field or variant holding it, where an attribute allowing a lint goes.
    pub(super) declared: OriginId,
}

impl Node {
    /// The nodes of the file `env` reads.
    pub(super) fn of(env: &Env<'_, '_>) -> Vec<Self> {
        let used = env.parsed.resolution.names.iter().filter_map(|(_, res)| match res {
            Some(Res::Binder(b)) => Some(*b),
            _ => None,
        });
        let mut walk = Walk {
            env,
            nodes: Vec::new(),
            allowed: Vec::new(),
            declared: None,
            ns: None,
            public: true,
            used: used.collect(),
        };
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
    /// The declaration or field walked.
    declared: Option<OriginId>,
    /// The namespace whose members are walked.
    ns: Option<NsId>,
    /// Whether the declarations around are seen from any package.
    public: bool,
    /// The binders the code names.
    used: HashSet<BinderId>,
}

impl Walk<'_, '_, '_> {
    fn hir(&self) -> &Hir {
        &self.env.parsed.hir
    }

    fn namespace(&mut self, ns: NsId) {
        let around = self.ns.replace(ns);
        let members: Vec<_> =
            self.env.parsed.hir.namespaces[ns].members.iter().filter_map(Entry::written).collect();
        for member in members {
            match member {
                Member::Decl(d) => self.decl(*d),
                Member::Field(f) => self.field(f),
                Member::Spread(_) => {}
            }
        }
        self.ns = around;
    }

    /// `f` run inside the declaration or field written at `declared`, with the lints its
    /// attributes `attrs` allow added to those allowed around.
    fn allowing(&mut self, declared: OriginId, attrs: &[ExprId], f: impl FnOnce(&mut Self)) {
        let (outer, around) = (self.allowed.len(), self.declared.replace(declared));
        let allows = self.env.attributes(attrs, Primitive::Lint(Lint::allow));
        self.allowed.extend(allows.into_iter().filter_map(|args| args.into_iter().next()));
        f(self);
        self.allowed.truncate(outer);
        self.declared = around;
    }

    fn push(&mut self, kind: Kind, origin: OriginId) {
        let declared = self.declared.unwrap_or(origin);
        self.nodes.push(Node { kind, origin, allowed: self.allowed.clone(), declared });
    }

    fn decl(&mut self, id: DeclId) {
        let d = &self.env.parsed.hir.decls[id];
        let (pat, value, documented) = (d.pat, d.value, !d.doc.is_empty());
        self.allowing(d.origin, &d.attrs, |walk| {
            let around = walk.public;
            if let PatKind::Bind(b) = walk.hir().pats[pat].kind {
                let binder = &walk.hir().binders[b];
                let (name, origin) = (binder.name, binder.origin);
                walk.public &= walk.seen(name, &walk.hir().decls[id].attrs);
                let binds = walk.binds(id);
                walk.push(Kind::Decl { name, binds, public: walk.public, documented }, origin);
            }
            if let Some(value) = value {
                walk.value(value);
            }
            walk.public = around;
        });
    }

    /// A field: one holding a type binds a type.
    fn field(&mut self, f: &FieldDef) {
        self.allowing(f.origin, &f.attrs, |walk| {
            if let Name::Plain(name) = f.name {
                let binds = if walk.types(f.ty) { Binds::Type } else { Binds::Field };
                let public = walk.public && walk.seen(name, &f.attrs);
                walk.push(Kind::Decl { name, binds, public, documented: !f.doc.is_empty() }, f.origin);
            }
        });
    }

    /// A variant, seen wherever its enum is.
    fn variant(&mut self, v: &Variant) {
        if let Name::Plain(name) = v.name {
            let around = self.declared.replace(v.origin);
            let kind = Kind::Decl {
                name,
                binds: Binds::Variant,
                public: self.public,
                documented: !v.doc.is_empty(),
            };
            self.push(kind, v.origin);
            self.declared = around;
        }
    }

    /// Whether the member `name` with attributes `attrs` is seen from any package.
    fn seen(&self, name: Symbol, attrs: &[ExprId]) -> bool {
        self.env.visible(name, attrs, self.env.home()).0.reach == Reach::Public
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
                let mut locals = Locals::default();
                locals.expr(self.hir(), expr);
                for Bound { binder, param, tests } in locals.found() {
                    let (name, origin) = (self.hir().binders[binder].name, self.hir().binders[binder].origin);
                    let (used, hides) = (self.used.contains(&binder), self.hides(name));
                    self.push(Kind::Local { name, used, param, tests, hides }, origin);
                }
            }
        }
    }

    /// What the declaration `id` binds (spec 10, section 5), by how it is written: a type when
    /// annotated `type`, `lang.Interface` or a function type giving one, or when its value is a
    /// `struct` that is no namespace, an `enum`, a function type, an `interface` or a function
    /// literal giving a type. Any other value, an alias of a type included, is annotated to bind
    /// one. A primitive, `= _`, binds what the annotation of its declaration says.
    fn binds(&self, id: DeclId) -> Binds {
        let hir = self.hir();
        let d = &hir.decls[id];
        let literal = |e: ExprId| Literal::written(self.env.cx, &ExprRef::new(self.env.file.clone(), hir, e));
        let annotated = d.ty.and_then(literal).is_some_and(|l| l.gives_type());
        let written = d.value.and_then(literal).is_some_and(|l| l.is_type());
        if annotated || written { Binds::Type } else { Binds::Value }
    }

    /// Whether a local named `name` in the namespace walked hides a constant from outside the file.
    fn hides(&self, name: Symbol) -> bool {
        let site = self.ns.and_then(|ns| self.env.outside(name, ns).ok());
        site.and_then(|site| self.env.value_of(site).ok()).is_some_and(|value| value.is_datum())
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
