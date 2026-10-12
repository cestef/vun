//! Walking the HIR. A [`Visitor`] overrides the nodes it cares about and calls the matching
//! `walk_*` function to continue into their children, in source order.

use crate::tree::{
    Arg, BlockId, Cond, DeclId, Entry, ExprId, ExprKind, ForHead, Hir, List, Member, NsId, PatId, PatKind,
    Spread, Staged, Stmt,
};

pub trait Visitor {
    fn namespace(&mut self, hir: &Hir, ns: NsId) {
        walk_namespace(self, hir, ns);
    }
    fn decl(&mut self, hir: &Hir, decl: DeclId) {
        walk_decl(self, hir, decl);
    }
    fn block(&mut self, hir: &Hir, block: BlockId) {
        walk_block(self, hir, block);
    }
    fn expr(&mut self, hir: &Hir, expr: ExprId) {
        walk_expr(self, hir, expr);
    }
    fn pat(&mut self, hir: &Hir, pat: PatId) {
        walk_pat(self, hir, pat);
    }
    fn spread(&mut self, hir: &Hir, spread: &Spread) {
        self.expr(hir, spread.path);
    }
    /// The namespace of a spread written in a block, entered where it is written.
    fn spread_scope(&mut self, hir: &Hir, ns: NsId) {
        self.namespace(hir, ns);
    }
    /// A `$for` or `$if` in a list; `entry` visits one entry of its body.
    fn staged<T>(&mut self, hir: &Hir, staged: &Staged<T>, entry: &mut dyn FnMut(&mut Self, &T)) {
        walk_staged(self, hir, staged, entry);
    }
}

/// Visits every expression of `ids`, such as an optional one.
fn each<'h, V: Visitor + ?Sized>(v: &mut V, hir: &Hir, ids: impl IntoIterator<Item = &'h ExprId>) {
    for &e in ids {
        v.expr(hir, e);
    }
}

pub fn walk_list<V: Visitor + ?Sized, T>(
    v: &mut V,
    hir: &Hir,
    list: &List<T>,
    entry: &mut dyn FnMut(&mut V, &T),
) {
    for e in list {
        match e {
            Entry::One(item) => entry(v, item),
            Entry::Staged(staged) => v.staged(hir, staged, entry),
        }
    }
}

pub fn walk_staged<V: Visitor + ?Sized, T>(
    v: &mut V,
    hir: &Hir,
    staged: &Staged<T>,
    entry: &mut dyn FnMut(&mut V, &T),
) {
    match staged {
        Staged::For { pat, iter, body, .. } => {
            v.expr(hir, *iter);
            v.pat(hir, *pat);
            walk_list(v, hir, body, entry);
        }
        Staged::If { cond, then, otherwise, .. } => {
            walk_cond(v, hir, cond);
            walk_list(v, hir, then, entry);
            walk_list(v, hir, otherwise, entry);
        }
    }
}

pub fn walk_namespace<V: Visitor + ?Sized>(v: &mut V, hir: &Hir, ns: NsId) {
    walk_list(v, hir, &hir.namespaces[ns].members, &mut |v, m| match m {
        Member::Decl(d) => v.decl(hir, *d),
        Member::Spread(s) => v.spread(hir, s),
        Member::Field(f) => {
            each(v, hir, &f.attrs);
            if let crate::tree::Name::Computed(e) = f.name {
                v.expr(hir, e);
            }
            v.expr(hir, f.ty);
            each(v, hir, &f.default);
        }
    });
}

pub fn walk_decl<V: Visitor + ?Sized>(v: &mut V, hir: &Hir, decl: DeclId) {
    let d = &hir.decls[decl];
    each(v, hir, &d.attrs);
    each(v, hir, &d.ty);
    each(v, hir, &d.value);
    v.pat(hir, d.pat);
}

pub fn walk_block<V: Visitor + ?Sized>(v: &mut V, hir: &Hir, block: BlockId) {
    let b = &hir.blocks[block];
    walk_list(v, hir, &b.stmts, &mut |v, s| match s {
        Stmt::Decl(d) => v.decl(hir, *d),
        Stmt::Expr(e) => v.expr(hir, *e),
        Stmt::Assign { place, value, .. } => {
            v.expr(hir, *place);
            v.expr(hir, *value);
        }
        Stmt::Block(b) => v.block(hir, *b),
        Stmt::Spread(ns) => v.spread_scope(hir, *ns),
    });
    each(v, hir, &b.tail);
}

pub fn walk_cond<V: Visitor + ?Sized>(v: &mut V, hir: &Hir, cond: &Cond) {
    match cond {
        Cond::Expr(e) => v.expr(hir, *e),
        Cond::Test { pat, value } => {
            v.expr(hir, *value);
            v.pat(hir, *pat);
        }
    }
}

fn args<V: Visitor + ?Sized>(v: &mut V, hir: &Hir, list: &List<Arg>) {
    walk_list(v, hir, list, &mut |v, a| {
        if let Some(crate::tree::Name::Computed(e)) = a.name {
            v.expr(hir, e);
        }
        v.expr(hir, a.value);
    });
}

fn exprs<V: Visitor + ?Sized>(v: &mut V, hir: &Hir, list: &List<ExprId>) {
    walk_list(v, hir, list, &mut |v, e| v.expr(hir, *e));
}

#[allow(clippy::too_many_lines, reason = "one arm per kind of expression")]
pub fn walk_expr<V: Visitor + ?Sized>(v: &mut V, hir: &Hir, expr: ExprId) {
    use crate::tree::Name::Computed;
    match &hir.exprs[expr].kind {
        ExprKind::Error
        | ExprKind::Hole
        | ExprKind::Rest
        | ExprKind::Name(_)
        | ExprKind::Lit(_)
        | ExprKind::Continue { .. } => {}
        ExprKind::DotName(name) => {
            if let Computed(e) = name {
                v.expr(hir, *e);
            }
        }
        ExprKind::Member { object, name } => {
            v.expr(hir, *object);
            if let Computed(e) = name {
                v.expr(hir, *e);
            }
        }
        ExprKind::Call { callee, args: a } => {
            v.expr(hir, *callee);
            args(v, hir, a);
        }
        ExprKind::Index { object, index } => {
            v.expr(hir, *object);
            v.expr(hir, *index);
        }
        ExprKind::StructLit { ty, fields } => {
            each(v, hir, ty);
            args(v, hir, fields);
        }
        ExprKind::Tuple(items) | ExprKind::Array(items) => exprs(v, hir, items),
        ExprKind::Repeat { value: a, count: b } | ExprKind::ArrayType { len: a, elem: b } => {
            v.expr(hir, *a);
            v.expr(hir, *b);
        }
        ExprKind::Prefix { operand, .. }
        | ExprKind::Borrow { operand, .. }
        | ExprKind::Postfix { operand, .. } => {
            v.expr(hir, *operand);
        }
        ExprKind::Infix { left, right, .. } => {
            v.expr(hir, *left);
            each(v, hir, right);
        }
        ExprKind::Block(b) | ExprKind::Quote(b) => v.block(hir, *b),
        ExprKind::If { cond, then, otherwise } => {
            walk_cond(v, hir, cond);
            v.block(hir, *then);
            each(v, hir, otherwise);
        }
        ExprKind::For { head, body } => {
            match head {
                ForHead::Forever => {}
                ForHead::While(cond) => walk_cond(v, hir, cond),
                ForHead::In { pat, iter } => {
                    v.expr(hir, *iter);
                    v.pat(hir, *pat);
                }
            }
            v.block(hir, *body);
        }
        ExprKind::Match { scrutinee, arms } => {
            v.expr(hir, *scrutinee);
            walk_list(v, hir, arms, &mut |v, arm| {
                v.pat(hir, arm.pat);
                each(v, hir, &arm.guard);
                each(v, hir, &arm.value);
            });
        }
        ExprKind::Labeled { body, .. } | ExprKind::Yield(body) | ExprKind::Defer(body) => v.expr(hir, *body),
        ExprKind::Return(value) | ExprKind::Break { value, .. } => each(v, hir, value),
        ExprKind::Fn { params, returns, body } => {
            walk_params(v, hir, params);
            each(v, hir, returns);
            v.block(hir, *body);
        }
        ExprKind::FnType { params, returns } => {
            walk_params(v, hir, params);
            each(v, hir, returns);
        }
        ExprKind::Closure { captures, func } => {
            exprs(v, hir, captures);
            v.expr(hir, *func);
        }
        ExprKind::Struct(ns) => v.namespace(hir, *ns),
        ExprKind::Enum { variants, ns } => {
            walk_list(v, hir, variants, &mut |v, variant| {
                if let Computed(e) = variant.name {
                    v.expr(hir, e);
                }
                each(v, hir, &variant.attrs);
                each(v, hir, &variant.payload);
                each(v, hir, &variant.code);
            });
            v.namespace(hir, *ns);
        }
        ExprKind::Attributed { attrs, inner } => {
            each(v, hir, attrs);
            v.expr(hir, *inner);
        }
    }
}

pub fn walk_params<V: Visitor + ?Sized>(v: &mut V, hir: &Hir, params: &List<crate::tree::Param>) {
    walk_list(v, hir, params, &mut |v, p| {
        each(v, hir, &p.attrs);
        each(v, hir, &p.ty);
        p.pat.iter().for_each(|&pat| v.pat(hir, pat));
    });
}

pub fn walk_pat<V: Visitor + ?Sized>(v: &mut V, hir: &Hir, pat: PatId) {
    use crate::tree::Name::Computed;
    match &hir.pats[pat].kind {
        PatKind::Bind(_) | PatKind::Hole | PatKind::Rest(_) | PatKind::Error => {}
        PatKind::Or(alternatives) => alternatives.iter().for_each(|&p| v.pat(hir, p)),
        PatKind::Range { lo, hi, .. } => {
            v.expr(hir, *lo);
            each(v, hir, hi);
        }
        PatKind::At { pat, .. } => v.pat(hir, *pat),
        PatKind::Tuple(items) => walk_list(v, hir, items, &mut |v, p| v.pat(hir, *p)),
        PatKind::Variant { name, args } => {
            if let Computed(e) = name {
                v.expr(hir, *e);
            }
            if let Some(args) = args {
                walk_list(v, hir, args, &mut |v, p| v.pat(hir, *p));
            }
        }
        PatKind::Struct { ty, fields } => {
            each(v, hir, ty);
            walk_list(v, hir, fields, &mut |v, (name, p)| {
                if let Some(Computed(e)) = name {
                    v.expr(hir, *e);
                }
                v.pat(hir, *p);
            });
        }
        PatKind::Value(e) | PatKind::Computed(e) => v.expr(hir, *e),
    }
}
