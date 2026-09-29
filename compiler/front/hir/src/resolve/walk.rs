//! The resolving walk: scopes follow the tree, and each binder is checked as it is declared.

use vun_diag::{ChildKind, DiagCx, Diagnostic, Msg, errors};
use vun_grammar::TokenKind;
use vun_index::IndexVec;

use super::Res;
use super::scope::{Kind, Scopes};
use crate::tree::{
    BinderId, BlockId, Cond, DeclId, ExprId, ExprKind, ForHead, Hir, List, NsId, Param, PatId, PatKind,
    Staged,
};
use crate::visit::{self, Visitor};

pub(super) struct Resolver<'c, 'd> {
    pub(super) scopes: Scopes,
    pub(super) names: IndexVec<ExprId, Option<Res>>,
    pub(super) parents: IndexVec<NsId, Option<NsId>>,
    /// The namespaces around the current node, innermost last.
    pub(super) namespaces: Vec<NsId>,
    /// Whether visiting a pattern declares its binders; namespace members are declared up front.
    pub(super) declaring: bool,
    /// Whether the declarations visited are namespace members, declared up front, rather than
    /// block statements; `$for` bodies keep the value of their list.
    pub(super) members: bool,
    /// The quotes around the current node (spec 03, section 4).
    pub(super) quotes: usize,
    pub(super) cx: &'c mut DiagCx<'d>,
}

impl Resolver<'_, '_> {
    fn with_scope(&mut self, kind: Kind, body: impl FnOnce(&mut Self)) {
        self.scopes.push(kind);
        body(self);
        self.scopes.pop();
    }

    /// A test `p = e` binds `p` in the scope of what follows it; an irrefutable `p` is an error.
    fn cond(&mut self, hir: &Hir, cond: &Cond) {
        match cond {
            Cond::Expr(e) => self.expr(hir, *e),
            Cond::Test { pat, value } => {
                self.expr(hir, *value);
                if hir.pats[*pat].kind.irrefutable(&hir.pats) {
                    let msg = Msg::new().text("this pattern always matches, so the test is always true");
                    let help = Msg::new().text("bind it instead: ").code("p := e;");
                    let d = Diagnostic::error(errors::BINDING_IRREFUTABLE_TEST, msg)
                        .at(hir.pats[*pat].origin, None);
                    self.cx.diag.error(d.child(ChildKind::Help, help));
                }
                self.bind_pattern(hir, *pat);
            }
        }
    }

    /// A function literal: parameters declared in order in a scope that hides enclosing run-time
    /// locals; `captures` are the binders a closure lists, visible inside it.
    fn function(
        &mut self,
        hir: &Hir,
        params: &List<Param>,
        returns: Option<ExprId>,
        body: Option<BlockId>,
        captures: &[BinderId],
    ) {
        let kind = if body.is_some() { Kind::Function } else { Kind::Signature };
        self.with_scope(kind, |this| {
            for &c in captures {
                this.scopes.bind(hir.binders[c].name, c);
            }
            visit::walk_list(this, hir, params, &mut |this, p| {
                for &a in &p.attrs {
                    this.expr(hir, a);
                }
                if let Some(ty) = p.ty {
                    this.expr(hir, ty);
                }
                if let Some(pat) = p.pat {
                    this.bind_pattern(hir, pat);
                }
            });
            if let Some(r) = returns {
                this.expr(hir, r);
            }
            if let Some(b) = body {
                this.block(hir, b);
            }
        });
    }
}

impl Visitor for Resolver<'_, '_> {
    fn namespace(&mut self, hir: &Hir, ns: NsId) {
        self.parents[ns] = self.namespaces.last().copied();
        self.namespaces.push(ns);
        self.with_scope(Kind::Namespace, |this| {
            let members = hir.members(ns);
            for binder in members {
                this.declare(hir, binder);
            }
            let declaring = std::mem::replace(&mut this.declaring, false);
            let members = std::mem::replace(&mut this.members, true);
            visit::walk_namespace(this, hir, ns);
            this.declaring = declaring;
            this.members = members;
        });
        self.namespaces.pop();
    }

    fn decl(&mut self, hir: &Hir, decl: DeclId) {
        let d = &hir.decls[decl];
        // Namespace members are already declared. In a block a binding is visible after its value,
        // except a compile-time one, which is also visible in it, as members are.
        let mut binders = Vec::new();
        hir.binders(d.pat, &mut binders);
        let early = !self.members && !binders.is_empty() && binders.iter().all(|&b| hir.binders[b].comptime);
        if early {
            self.bind_pattern(hir, d.pat);
        }
        for &a in &d.attrs {
            self.expr(hir, a);
        }
        if let Some(ty) = d.ty {
            self.expr(hir, ty);
        }
        if let Some(value) = d.value {
            self.expr(hir, value);
        }
        if self.members {
            self.pat(hir, d.pat);
        } else if !early {
            self.bind_pattern(hir, d.pat);
        }
    }

    fn block(&mut self, hir: &Hir, block: BlockId) {
        let declaring = std::mem::replace(&mut self.declaring, false);
        let members = std::mem::replace(&mut self.members, false);
        self.with_scope(Kind::Local, |this| visit::walk_block(this, hir, block));
        self.declaring = declaring;
        self.members = members;
    }

    fn pat(&mut self, hir: &Hir, pat: PatId) {
        if let PatKind::Or(alternatives) = &hir.pats[pat].kind {
            self.alternatives(hir, alternatives);
        }
        if self.declaring {
            match &hir.pats[pat].kind {
                PatKind::Bind(b) | PatKind::Rest(Some(b)) | PatKind::At { binder: b, .. } => {
                    self.declare(hir, *b);
                }
                PatKind::Or(alternatives) => {
                    // The first alternative declares; the others bind the same names.
                    let mut rest = alternatives.iter();
                    if let Some(&first) = rest.next() {
                        self.pat(hir, first);
                    }
                    let declaring = std::mem::replace(&mut self.declaring, false);
                    rest.for_each(|&p| self.pat(hir, p));
                    self.declaring = declaring;
                    return;
                }
                _ => {}
            }
        }
        visit::walk_pat(self, hir, pat);
    }

    fn staged<T>(&mut self, hir: &Hir, staged: &Staged<T>, entry: &mut dyn FnMut(&mut Self, &T)) {
        match staged {
            Staged::For { pat, iter, body, .. } => {
                self.expr(hir, *iter);
                self.with_scope(Kind::Local, |this| {
                    this.bind_pattern(hir, *pat);
                    let declaring = std::mem::replace(&mut this.declaring, false);
                    visit::walk_list(this, hir, body, entry);
                    this.declaring = declaring;
                });
            }
            // A branch's declarations are its own, as a step's are: which branch is generated
            // is known only once the condition is.
            // What a test binds is in scope in the branch it leads to.
            Staged::If { cond, then, otherwise, .. } => {
                self.with_scope(Kind::Local, |this| {
                    this.cond(hir, cond);
                    visit::walk_list(this, hir, then, entry);
                });
                self.with_scope(Kind::Local, |this| visit::walk_list(this, hir, otherwise, entry));
            }
        }
    }

    fn expr(&mut self, hir: &Hir, expr: ExprId) {
        let declaring = std::mem::replace(&mut self.declaring, false);
        match &hir.exprs[expr].kind {
            ExprKind::Quote(block) => self.quoted(hir, *block),
            ExprKind::Prefix { op: TokenKind::Dollar, .. } => self.escape(hir, expr),
            ExprKind::Borrow { mode: None, .. } => {
                self.unknown_borrow(hir, expr);
                visit::walk_expr(self, hir, expr);
            }
            ExprKind::Name(name) => {
                let ns = *self.namespaces.last().expect("every expression is inside the file's namespace");
                let res = self.scopes.lookup(hir, *name).map_or(Res::Outer { name: *name, ns }, Res::Binder);
                self.names[expr] = Some(res);
            }
            ExprKind::If { cond, then, otherwise } => {
                self.with_scope(Kind::Local, |this| {
                    this.cond(hir, cond);
                    this.block(hir, *then);
                });
                if let Some(o) = otherwise {
                    self.expr(hir, *o);
                }
            }
            ExprKind::For { head, body } => self.with_scope(Kind::Local, |this| {
                match head {
                    ForHead::Forever => {}
                    ForHead::While(cond) => this.cond(hir, cond),
                    ForHead::In { pat, iter } => {
                        this.expr(hir, *iter);
                        this.bind_pattern(hir, *pat);
                    }
                }
                this.block(hir, *body);
            }),
            ExprKind::Match { scrutinee, arms } => {
                self.expr(hir, *scrutinee);
                visit::walk_list(self, hir, arms, &mut |this, arm| {
                    this.with_scope(Kind::Local, |this| {
                        this.bind_pattern(hir, arm.pat);
                        if let Some(g) = arm.guard {
                            this.expr(hir, g);
                        }
                        if let Some(v) = arm.value {
                            this.expr(hir, v);
                        }
                    });
                });
            }
            ExprKind::Fn { params, returns, body } => self.function(hir, params, *returns, Some(*body), &[]),
            ExprKind::FnType { params, returns } => self.function(hir, params, *returns, None, &[]),
            ExprKind::Closure { captures, func } => {
                let mut captured = Vec::new();
                visit::walk_list(self, hir, captures, &mut |this, &c| {
                    this.expr(hir, c);
                    if let Some(Res::Binder(b)) = this.captured(hir, c) {
                        captured.push(b);
                    }
                });
                match &hir.exprs[*func].kind {
                    ExprKind::Fn { params, returns, body } => {
                        self.function(hir, params, *returns, Some(*body), &captured);
                    }
                    _ => self.expr(hir, *func),
                }
            }
            _ => visit::walk_expr(self, hir, expr),
        }
        self.declaring = declaring;
    }
}

impl Resolver<'_, '_> {
    /// The binder a capture refers to: `x`, `&x` or `&mut x`.
    fn captured(&self, hir: &Hir, capture: ExprId) -> Option<Res> {
        match &hir.exprs[capture].kind {
            ExprKind::Name(_) => self.names[capture],
            ExprKind::Borrow { operand, .. } => self.captured(hir, *operand),
            _ => None,
        }
    }
}
