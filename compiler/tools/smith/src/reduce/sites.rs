//! The walk over a program that counts its sites and applies one.

use crate::ir::{Expr, Kind, Program, Role, Stmt};

/// A walk that numbers every way to shrink what it passes, and applies the one it was asked for.
pub(super) struct Sites {
    seen: usize,
    pick: Option<usize>,
    applied: bool,
}

impl Sites {
    pub(super) const fn counting() -> Self {
        Self { seen: 0, pick: None, applied: false }
    }

    pub(super) const fn picking(nth: usize) -> Self {
        Self { seen: 0, pick: Some(nth), applied: false }
    }

    pub(super) const fn seen(&self) -> usize {
        self.seen
    }

    pub(super) const fn applied(&self) -> bool {
        self.applied
    }

    /// Whether the site just reached is the one to apply.
    fn hit(&mut self) -> bool {
        let n = self.seen;
        self.seen += 1;
        let hit = self.pick == Some(n);
        self.applied |= hit;
        hit
    }

    pub(super) fn program(&mut self, program: &mut Program) {
        let mut at = 0;
        while at < program.roots.len() {
            if program.roots.len() > 1 && self.hit() {
                program.roots.remove(at);
                return;
            }
            at += 1;
        }
        for (_, function) in program.functions.iter_mut() {
            // An iterator keeps its first statement, the yield that makes it one.
            let kept = usize::from(function.role == Role::Iterator);
            self.block_after(&mut function.body, kept);
            self.expr(&mut function.tail);
        }
    }

    fn block(&mut self, stmts: &mut Vec<Stmt>) {
        self.block_after(stmts, 0);
    }

    /// A block whose first `kept` statements are never removed, only shrunk.
    fn block_after(&mut self, stmts: &mut Vec<Stmt>, kept: usize) {
        let mut at = 0;
        while at < stmts.len() {
            if at >= kept && self.hit() {
                stmts.remove(at);
                return;
            }
            if let Stmt::If { then, otherwise, .. } = &mut stmts[at] {
                if self.hit() {
                    let body = std::mem::take(then);
                    stmts.splice(at..=at, body);
                    return;
                }
                if self.hit() {
                    let body = std::mem::take(otherwise);
                    stmts.splice(at..=at, body);
                    return;
                }
            }
            self.stmt(&mut stmts[at]);
            at += 1;
        }
    }

    fn stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::Let { init, .. } => self.expr(init),
            Stmt::Assign { place, value, .. } => {
                for index in place.indices_mut() {
                    self.expr(index);
                }
                self.expr(value);
            }
            Stmt::If { condition, then, otherwise } => {
                self.expr(condition);
                self.block(then);
                self.block(otherwise);
            }
            Stmt::Count { count, body, .. } => {
                if *count > 1 && self.hit() {
                    *count = 1;
                    return;
                }
                self.block(body);
            }
            Stmt::Each { body, .. } => self.block(body),
            Stmt::While { condition, body } => {
                self.expr(condition);
                self.block(body);
            }
            Stmt::WhileSome { option, body, .. } => {
                self.expr(option);
                self.block(body);
            }
            Stmt::Drain { args, body, .. } => {
                for arg in args {
                    self.expr(arg);
                }
                self.block(body);
            }
            Stmt::Yield(value) => self.expr(value),
            Stmt::Defer(inner) | Stmt::Labeled(_, inner) => self.stmt(inner),
            Stmt::Break(_, value) => {
                if let Some(value) = value {
                    self.expr(value);
                }
            }
            Stmt::Continue(_) => {}
        }
    }

    fn expr(&mut self, expr: &mut Expr) {
        if !expr.is_sample() && self.hit() {
            *expr = Expr::sample(&expr.ty);
            return;
        }
        let ty = expr.ty.clone();
        for at in 0..expr.children_mut().len() {
            if expr.children_mut()[at].ty == ty && self.hit() {
                let child = std::mem::replace(expr.children_mut()[at], Expr::unit());
                *expr = child;
                return;
            }
        }
        if let Kind::Block(stmts, _) | Kind::Forever(_, stmts) = &mut expr.kind {
            self.block(stmts);
        }
        for child in expr.children_mut() {
            self.expr(child);
        }
    }
}
