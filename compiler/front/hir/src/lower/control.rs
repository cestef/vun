//! Control flow and functions.

use vun_grammar::{NodeField, NodeKind};
use vun_syntax::cst::{Fields, NodeId};

use super::Lower;
use super::pats::Mode;
use crate::tree::{Arm, Cond, Expr, ExprId, ExprKind, ForHead, List, Param};

impl Lower<'_, '_> {
    pub(super) fn control(&mut self, f: &Fields, n: NodeId) -> ExprKind {
        match self.kind(n) {
            NodeKind::If => self.if_kind(f, n, Mode::PLAIN),
            NodeKind::For => {
                let head = match f.one(NodeField::Head) {
                    None => ForHead::Forever,
                    Some(h) if self.kind(h) == NodeKind::In => {
                        let hf = self.fields(h);
                        let pat = self.pat_at(&hf, h, NodeField::Pattern, Mode::PLAIN);
                        let iter = self.operand(&hf, h, NodeField::Value);
                        ForHead::In { pat, iter }
                    }
                    Some(h) => ForHead::While(self.cond(h, Mode::PLAIN)),
                };
                let body = self.block_at(f, n, NodeField::Block);
                ExprKind::For { head, body }
            }
            NodeKind::Match => {
                let scrutinee = self.operand(f, n, NodeField::Scrutinee);
                ExprKind::Match { scrutinee, arms: self.arms(f.get(NodeField::Arms)) }
            }
            NodeKind::Labeled => {
                let label = self.label(n);
                let body = self.operand(f, n, NodeField::Body);
                ExprKind::Labeled { label, body }
            }
            NodeKind::Return => ExprKind::Return(f.one(NodeField::Value).map(|v| self.expr(v))),
            NodeKind::Break => {
                let label = f.one(NodeField::Label).map(|l| self.label(l));
                ExprKind::Break { label, value: f.one(NodeField::Value).map(|v| self.expr(v)) }
            }
            NodeKind::Continue => {
                ExprKind::Continue { label: f.one(NodeField::Label).map(|l| self.label(l)) }
            }
            NodeKind::Yield => ExprKind::Yield(self.operand(f, n, NodeField::Value)),
            NodeKind::Defer => ExprKind::Defer(self.operand(f, n, NodeField::Body)),
            NodeKind::Fn => {
                let params = self.params(f.get(NodeField::Params), true);
                let returns = self.returns(f);
                let body = self.block_at(f, n, NodeField::Block);
                ExprKind::Fn { params, returns, body }
            }
            NodeKind::FnType => {
                let params = self.params(f.get(NodeField::Params), false);
                ExprKind::FnType { params, returns: self.returns(f) }
            }
            _ => ExprKind::Error,
        }
    }

    /// An `if` whose tests bind as `mode` says, its `else if`s alike: at compile time for a
    /// `$if`'s.
    fn if_kind(&mut self, f: &Fields, n: NodeId, mode: Mode) -> ExprKind {
        let cond = match f.one(NodeField::Condition) {
            Some(c) => self.cond(c, mode),
            None => Cond::Expr(self.error(n)),
        };
        let then = self.block_at(f, n, NodeField::Block);
        let otherwise = f.one(NodeField::Otherwise).map(|o| match self.kind(o) {
            NodeKind::If => self.if_expr(o, mode),
            _ => self.expr(o),
        });
        ExprKind::If { cond, then, otherwise }
    }

    /// The `if` at `n`, its tests binding as `mode` says.
    pub(super) fn if_expr(&mut self, n: NodeId, mode: Mode) -> ExprId {
        let f = self.fields(n);
        let kind = self.if_kind(&f, n, mode);
        let origin = self.origin(n);
        self.hir.exprs.push(Expr { kind, origin })
    }

    /// An `if` or `for` head: a condition, or a test `pattern = value` whose binders are bound
    /// as `mode` says: at compile time for a `$if`'s.
    pub(super) fn cond(&mut self, n: NodeId, mode: Mode) -> Cond {
        if self.kind(n) != NodeKind::Test {
            return Cond::Expr(self.expr(n));
        }
        let f = self.fields(n);
        let pat = self.pat_at(&f, n, NodeField::Pattern, mode);
        let value = self.operand(&f, n, NodeField::Value);
        Cond::Test { pat, value }
    }

    fn returns(&mut self, f: &Fields) -> Option<crate::tree::ExprId> {
        let returns = f.one(NodeField::Returns)?;
        let rf = self.fields(returns);
        Some(self.operand(&rf, returns, NodeField::Type))
    }

    /// Parameters: in a function literal every entry is a pattern, typed or not; in a function
    /// type an untyped entry is a type.
    fn params(&mut self, nodes: &[NodeId], literal: bool) -> List<Param> {
        let nodes: Vec<_> = nodes.iter().copied().filter(|&n| !self.filler(n)).collect();
        self.entries(nodes, &mut |this, n| {
            let (inner, _, attrs) = this.annotations(n);
            let origin = this.origin(inner);
            Some(if this.kind(inner) == NodeKind::Param {
                let f = this.fields(inner);
                let pat = f.one(NodeField::Pattern).map(|p| this.pattern(p, Mode::PLAIN));
                let ty = f.one(NodeField::Type).map(|t| this.expr(t));
                Param { pat, ty, attrs, origin }
            } else if literal {
                Param { pat: Some(this.pattern(inner, Mode::PLAIN)), ty: None, attrs, origin }
            } else {
                Param { pat: None, ty: Some(this.expr(inner)), attrs, origin }
            })
        })
    }

    fn arms(&mut self, nodes: &[NodeId]) -> List<Arm> {
        self.entries(nodes.to_vec(), &mut |this, n| {
            if this.kind(n) != NodeKind::Arm {
                return None;
            }
            let f = this.fields(n);
            let pat = this.pat_at(&f, n, NodeField::Pattern, Mode::PLAIN);
            let guard = f
                .one(NodeField::Guard)
                .and_then(|g| this.fields(g).one(NodeField::Condition))
                .map(|c| this.expr(c));
            let value = f.one(NodeField::Value).map(|v| this.expr(v));
            Some(Arm { pat, guard, value, origin: this.origin(n) })
        })
    }
}
