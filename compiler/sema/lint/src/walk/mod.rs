//! Walking a file for the nodes lints look at (spec 10, section 4), carrying the lints
//! `#[lint.allow]` silences: on a declaration, a field, a local binding, or an expression.

mod binds;

use std::collections::HashSet;

use vun_hir::resolve::Res;
use vun_hir::tree::{
    BinderId, Cond, DeclId, Entry, ExprId, ExprKind, FieldDef, ForHead, Hir, Joined, List, Member, Name,
    NsId, Param, PatId, PatKind, Variant, each_entry,
};
use vun_hir::visit::{self, Visitor};
use vun_intern::Symbol;
use vun_origin::OriginId;
use vun_primitives::Primitive;
use vun_primitives::lint::Lint;
use vun_sema_ty::Value;

use super::nodes::{Kind, Node};
use super::tables::Binds;
use vun_sema::env::Env;
use vun_sema::visible::Reach;

/// Walking the file `env` reads: its declarations and the locals and calls in their values.
pub(super) struct Walk<'e, 'a, 'db> {
    env: &'e Env<'a, 'db>,
    nodes: Vec<Node>,
    /// The lints allowed by the declarations and expressions around the one walked.
    allowed: Vec<Value>,
    /// The declaration, field or local whose attributes allow a lint, where a fix goes.
    declared: Option<OriginId>,
    /// The namespace whose members are walked.
    ns: Option<NsId>,
    /// Whether the declarations around are seen from any package.
    public: bool,
    /// The binders the code names.
    used: HashSet<BinderId>,
    /// The binders of function literal parameters.
    params: HashSet<BinderId>,
    /// The binders of a pattern testing the value it matches.
    tested: HashSet<BinderId>,
    /// Binders that are no local of their own.
    skipped: HashSet<BinderId>,
}

impl<'e, 'a, 'db> Walk<'e, 'a, 'db> {
    /// The nodes of the file `env` reads.
    pub(super) fn of(env: &'e Env<'a, 'db>) -> Vec<Node> {
        let used = env.parsed.resolution.names.iter().filter_map(|(_, res)| match res {
            Some(Res::Binder(b)) => Some(*b),
            _ => None,
        });
        let mut walk = Self {
            env,
            nodes: Vec::new(),
            allowed: Vec::new(),
            declared: None,
            ns: None,
            public: true,
            used: used.collect(),
            params: HashSet::new(),
            tested: HashSet::new(),
            skipped: HashSet::new(),
        };
        if let Some(root) = env.parsed.hir.root {
            walk.members(&env.parsed.hir, root);
        }
        walk.nodes
    }

    fn hir(&self) -> &Hir {
        &self.env.parsed.hir
    }

    /// `f` run with the lints `attrs` allow added to those allowed around, reported at
    /// `declared`.
    fn allowing(&mut self, declared: OriginId, attrs: &[ExprId], f: impl FnOnce(&mut Self)) {
        let (outer, around) = (self.allowed.len(), self.declared.replace(declared));
        self.allow(attrs);
        f(self);
        self.allowed.truncate(outer);
        self.declared = around;
    }

    /// `f` run with the lints `attrs` allow added to those allowed around.
    fn scope(&mut self, attrs: &[ExprId], f: impl FnOnce(&mut Self)) {
        let outer = self.allowed.len();
        self.allow(attrs);
        f(self);
        self.allowed.truncate(outer);
    }

    /// Adds the `#[lint.allow(lint, ..)]` of `attrs` to those allowed around.
    fn allow(&mut self, attrs: &[ExprId]) {
        let allows = self.env.attributes(attrs, Primitive::Lint(Lint::allow));
        self.allowed.extend(allows.into_iter().filter_map(|args| args.into_iter().next()));
    }

    fn push(&mut self, kind: Kind, origin: OriginId) {
        let declared = self.declared.unwrap_or(origin);
        self.nodes.push(Node { kind, origin, allowed: self.allowed.clone(), declared });
    }

    /// A local binding or a parameter: the lint `#[lint.allow]` around its declaration is in
    /// effect for it and for the calls in its value.
    fn local(&mut self, hir: &Hir, binder: BinderId, tests: bool) {
        let (name, origin) = (hir.binders[binder].name, hir.binders[binder].origin);
        let (used, hides) = (self.used.contains(&binder), self.hides(name));
        let param = self.params.contains(&binder);
        self.push(Kind::Local { name, used, param, tests, hides }, origin);
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

    /// Whether a local named `name` in the namespace walked hides a constant from outside the file.
    fn hides(&self, name: Symbol) -> bool {
        let site = self.ns.and_then(|ns| self.env.outside(name, ns).ok());
        site.and_then(|site| self.env.value_of(site).ok()).is_some_and(|value| value.is_datum())
    }

    /// The binders of the parameters `params`.
    fn param_binders(hir: &Hir, params: &List<Param>) -> Vec<BinderId> {
        let mut binders = Vec::new();
        each_entry(params, &mut |p: &Param| {
            if let Some(pat) = p.pat {
                hir.binders(pat, &mut binders);
            }
        });
        binders
    }

    /// The binders of the pattern `pat`, which tests the value it matches.
    fn mark_tested(&mut self, hir: &Hir, pat: PatId) {
        let mut binders = Vec::new();
        hir.binders(pat, &mut binders);
        self.tested.extend(binders);
    }

    /// The members of a namespace or a type body, walked as declarations of their own.
    fn members(&mut self, hir: &Hir, ns: NsId) {
        let around = self.ns.replace(ns);
        let members: Vec<_> = hir.namespaces[ns].members.iter().filter_map(Entry::written).collect();
        for member in members {
            match member {
                Member::Decl(d) => self.decl(hir, *d),
                Member::Field(f) => self.field(f),
                Member::Spread(_) => {}
            }
        }
        self.ns = around;
    }

    /// A declaration's value: a `struct` or `enum` literal is a namespace of its own, whose
    /// members are declarations; any other value holds locals and calls.
    fn value(&mut self, hir: &Hir, expr: ExprId) {
        match &hir.exprs[expr].kind {
            ExprKind::Struct(ns) => self.members(hir, *ns),
            ExprKind::Enum { variants, ns } => {
                for v in variants.iter().filter_map(Entry::written) {
                    self.variant(v);
                }
                self.members(hir, *ns);
            }
            ExprKind::Attributed { attrs, inner } => self.scope(attrs, |walk| walk.value(hir, *inner)),
            _ => self.expr(hir, expr),
        }
    }
}

impl Visitor for Walk<'_, '_, '_> {
    /// A namespace inside a value is a type's, whose members are declarations of their own; only
    /// a declaration's own value is one, so nothing is walked here.
    fn namespace(&mut self, _: &Hir, _: NsId) {}

    /// A declaration: a member of a namespace or a type, or a local binding, each walking its
    /// value inside the lints its attributes allow.
    fn decl(&mut self, hir: &Hir, id: DeclId) {
        let d = &hir.decls[id];
        let (pat, value, documented) = (d.pat, d.value, !d.doc.is_empty());
        let local = matches!(hir.pats[pat].kind, PatKind::Bind(b) if hir.is_local(b));
        if local {
            self.allowing(d.origin, &d.attrs, |walk| {
                walk.pat(hir, pat);
                if let Some(value) = value {
                    walk.expr(hir, value);
                }
            });
            return;
        }
        self.allowing(d.origin, &d.attrs, |walk| {
            let around = walk.public;
            if let PatKind::Bind(b) = hir.pats[pat].kind {
                let binder = &hir.binders[b];
                let (name, origin) = (binder.name, binder.origin);
                walk.public &= walk.seen(name, &hir.decls[id].attrs);
                let binds = walk.binds(id);
                walk.push(Kind::Decl { name, binds, public: walk.public, documented }, origin);
            }
            if let Some(value) = value {
                walk.value(hir, value);
            }
            walk.public = around;
        });
    }

    fn expr(&mut self, hir: &Hir, expr: ExprId) {
        match &hir.exprs[expr].kind {
            // Only a path names a function worth knowing; any other callee, a method called on a
            // call included, is not run.
            ExprKind::Call { callee, .. } => {
                if hir.path(*callee).is_some()
                    && let Ok(value) = self.env.eval(*callee)
                {
                    self.push(Kind::Call { callee: value }, hir.exprs[expr].origin);
                }
            }
            ExprKind::Attributed { attrs, inner } => {
                self.scope(attrs, |walk| walk.expr(hir, *inner));
                return;
            }
            ExprKind::Fn { params, .. } => self.params.extend(Self::param_binders(hir, params)),
            ExprKind::FnType { params, .. } => self.skipped.extend(Self::param_binders(hir, params)),
            ExprKind::If { cond: Cond::Test { pat, .. }, .. }
            | ExprKind::For { head: ForHead::While(Cond::Test { pat, .. }), .. } => {
                self.mark_tested(hir, *pat);
            }
            ExprKind::Match { arms, .. } => {
                let mut pats = Vec::new();
                each_entry(arms, &mut |arm| pats.push(arm.pat));
                for pat in pats {
                    self.mark_tested(hir, pat);
                }
            }
            _ => {}
        }
        visit::walk_expr(self, hir, expr);
    }

    fn pat(&mut self, hir: &Hir, pat: PatId) {
        if let PatKind::Bind(b) | PatKind::Rest(Some(b)) | PatKind::At { binder: b, .. } = &hir.pats[pat].kind
            && !self.skipped.contains(b)
        {
            self.local(hir, *b, self.tested.contains(b));
        }
        if let PatKind::Or(alternatives) = &hir.pats[pat].kind {
            let others = hir.joined(alternatives).into_iter().filter_map(|j| match j {
                Joined::Paired { other, .. } | Joined::Extra { other } => Some(other),
                Joined::Missing { .. } => None,
            });
            self.skipped.extend(others);
        }
        visit::walk_pat(self, hir, pat);
    }
}
