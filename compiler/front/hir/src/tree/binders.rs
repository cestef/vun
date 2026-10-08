//! The names a pattern or a namespace introduces.

use vun_intern::Symbol;

use super::{
    BinderId, DeclId, Entry, ExprId, ExprKind, FieldDef, Hir, List, Member, Name, NsId, PatId, PatKind,
    Spread, Staged, Variant,
};

impl Hir {
    /// Binders introduced by a pattern, without visiting the expressions inside it.
    pub fn binders(&self, pat: PatId, out: &mut Vec<BinderId>) {
        match &self.pats[pat].kind {
            PatKind::Bind(b) | PatKind::Rest(Some(b)) => out.push(*b),
            PatKind::At { binder, pat } => {
                out.push(*binder);
                self.binders(*pat, out);
            }
            PatKind::Tuple(items) => each_entry(items, &mut |p| self.binders(*p, out)),
            PatKind::Variant { args: Some(args), .. } => each_entry(args, &mut |p| self.binders(*p, out)),
            PatKind::Struct { fields, .. } => each_entry(fields, &mut |(_, p)| self.binders(*p, out)),
            // Alternatives bind the same names; the first stands for all.
            PatKind::Or(alternatives) => {
                if let Some(&first) = alternatives.first() {
                    self.binders(first, out);
                }
            }
            PatKind::Variant { args: None, .. }
            | PatKind::Rest(None)
            | PatKind::Hole
            | PatKind::Range { .. }
            | PatKind::Value(_)
            | PatKind::Computed(_)
            | PatKind::Error => {}
        }
    }

    /// The spreads among the members of `ns`.
    #[must_use]
    pub fn spreads(&self, ns: NsId) -> Vec<&Spread> {
        let mut out = Vec::new();
        each_entry(&self.namespaces[ns].members, &mut |m| {
            if let Member::Spread(s) = m {
                out.push(s);
            }
        });
        out
    }

    /// The declaration among the members of `ns` that binds `name`.
    #[must_use]
    pub fn declaring(&self, ns: NsId, name: Symbol) -> Option<DeclId> {
        self.member_decls(ns).into_iter().find(|&d| self.binds(d, |b| self.binders[b].name == name))
    }

    /// The member declaration of some namespace that binds `binder`; `None` for a local.
    #[must_use]
    pub fn member_decl(&self, binder: BinderId) -> Option<DeclId> {
        self.index().member[binder].map(|(_, decl)| decl)
    }

    /// Whether `binder` is a local, bound by a body: not a member of a namespace, declared in it
    /// or selected by a spread.
    #[must_use]
    pub fn is_local(&self, binder: BinderId) -> bool {
        self.member_decl(binder).is_none() && self.spread_of(binder).is_none()
    }

    /// Whether `pat` binds a `$` parameter.
    #[must_use]
    pub fn is_comptime(&self, pat: PatId) -> bool {
        matches!(self.pats[pat].kind, PatKind::Bind(b) if self.binders[b].comptime)
    }

    /// The namespace among whose members `binder` is declared; `None` for a local.
    #[must_use]
    pub fn member_ns(&self, binder: BinderId) -> Option<NsId> {
        self.index().member[binder].map(|(ns, _)| ns)
    }

    /// The `struct` literal whose members `ns` holds, if it is one.
    #[must_use]
    pub fn struct_of(&self, ns: NsId) -> Option<ExprId> {
        self.index().literal[ns]
    }

    /// The variants of the `enum` literal `decl` binds, staged ones included; none for any other
    /// value.
    #[must_use]
    pub fn variants_of(&self, decl: DeclId) -> Vec<&Variant> {
        let mut out = Vec::new();
        if let Some(ExprKind::Enum { variants, .. }) = self.decls[decl].value.map(|v| &self.exprs[v].kind) {
            each_entry(variants, &mut |v| out.push(v));
        }
        out
    }

    /// The path of the spread selecting `binder`, as `..std.mem.{ Own }` selects `Own`.
    #[must_use]
    pub fn spread_of(&self, binder: BinderId) -> Option<ExprId> {
        self.index().spread[binder]
    }

    /// The declaration binding `binder`, among members or in a block.
    #[must_use]
    pub fn decl_of(&self, binder: BinderId) -> Option<DeclId> {
        self.index().decl[binder]
    }

    /// The declarations among the members of `ns`, including staged ones.
    #[must_use]
    pub fn member_decls(&self, ns: NsId) -> Vec<DeclId> {
        let mut out = Vec::new();
        each_entry(&self.namespaces[ns].members, &mut |m| {
            if let Member::Decl(d) = m {
                out.push(*d);
            }
        });
        out
    }

    /// Whether the pattern of `decl` binds a binder satisfying `test`.
    fn binds(&self, decl: DeclId, test: impl Fn(BinderId) -> bool) -> bool {
        let mut binders = Vec::new();
        self.binders(self.decls[decl].pat, &mut binders);
        binders.into_iter().any(test)
    }

    /// The binders declared by the members of a namespace, including staged ones.
    #[must_use]
    pub fn members(&self, ns: NsId) -> Vec<BinderId> {
        let mut out = Vec::new();
        each_entry(&self.namespaces[ns].members, &mut |m| match m {
            Member::Decl(d) => self.binders(self.decls[*d].pat, &mut out),
            Member::Spread(s) => out.extend(s.select.iter().flatten()),
            Member::Field(_) => {}
        });
        out
    }
}

impl Hir {
    /// The field of the namespace `ns` written with the plain name `name`, staged ones included.
    #[must_use]
    pub fn field(&self, ns: NsId, name: Symbol) -> Option<&FieldDef> {
        let mut found = None;
        each_entry(&self.namespaces[ns].members, &mut |m| {
            if let Member::Field(f) = m
                && f.name == Name::Plain(name)
            {
                found = found.or(Some(f));
            }
        });
        found
    }

    /// Whether no member of `ns` can be named `name`: each is written under a plain name of its
    /// own, staged ones included. A spread or a computed name could bring any.
    #[must_use]
    pub fn lacks(&self, ns: NsId, name: Symbol) -> bool {
        let mut lacks = true;
        each_entry(&self.namespaces[ns].members, &mut |member| match member {
            Member::Decl(decl) => {
                let mut binders = Vec::new();
                self.binders(self.decls[*decl].pat, &mut binders);
                let computed = matches!(self.pats[self.decls[*decl].pat].kind, PatKind::Computed(_));
                lacks &=
                    !computed && !binders.is_empty() && binders.iter().all(|&b| self.binders[b].name != name);
            }
            Member::Field(field) => lacks &= matches!(field.name, Name::Plain(written) if written != name),
            Member::Spread(_) => lacks = false,
        });
        lacks
    }

    /// The body of the `struct` that `value` makes: the literal itself, or the end of a function
    /// literal's body.
    #[must_use]
    pub fn made_struct(&self, value: ExprId) -> Option<NsId> {
        match self.exprs[value].kind {
            ExprKind::Struct(ns) => Some(ns),
            ExprKind::Fn { body, .. } => self.made_struct(self.blocks[body].tail?),
            _ => None,
        }
    }
}

/// Calls `f` on every entry of `list`, entering both branches of `$if` and the body of `$for`.
pub fn each_entry<'a, T>(list: &'a List<T>, f: &mut dyn FnMut(&'a T)) {
    for e in list {
        match e {
            Entry::One(item) => f(item),
            Entry::Staged(Staged::For { body, .. }) => each_entry(body, f),
            Entry::Staged(Staged::If { then, otherwise, .. }) => {
                each_entry(then, f);
                each_entry(otherwise, f);
            }
        }
    }
}
