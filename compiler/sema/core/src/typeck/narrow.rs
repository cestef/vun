//! Comparisons narrow (spec 04, rule 4): where a comparison of integers is known to hold, an
//! immutable binding it compares has the range the comparison leaves it, in `if` branches, `for`
//! bodies, match arms, the right operand of `and` and `or`, and after an `if` whose branch cannot
//! fall through. Mutable bindings are never narrowed.

use vun_codec::{Decode, Encode};
use vun_desugar::COMPARISONS;
use vun_grammar::TokenKind;
use vun_hir::resolve::Res;
use vun_hir::tree::{BinderId, BlockId, Entry, ExprId, ExprKind, Stmt};
use vun_sema_int::{Compared, Interval};
use vun_sema_ty::TyKind;

use super::Typer;

/// The ranges bindings have where a condition holds, or where it fails.
pub(super) type Narrowed = Vec<(BinderId, Interval)>;

/// What a condition tells of immutable integer bindings.
#[derive(Default, Encode, Decode)]
pub(super) struct Facts {
    pub holds: Narrowed,
    pub fails: Narrowed,
}

impl Facts {
    fn negated(self) -> Self {
        Self { holds: self.fails, fails: self.holds }
    }
}

impl Typer<'_, '_, '_> {
    /// The facts of the condition `cond`, typed already.
    pub(super) fn facts(&self, cond: ExprId) -> Facts {
        match &self.hir().exprs[cond].kind {
            ExprKind::Prefix { op: TokenKind::Bang, operand, .. } => self.facts(*operand).negated(),
            ExprKind::Infix { op: TokenKind::And, left, right: Some(right) } => {
                let (left, right) = (self.facts(*left), self.facts(*right));
                Facts { holds: [left.holds, right.holds].concat(), fails: Narrowed::new() }
            }
            ExprKind::Infix { op: TokenKind::Or, left, right: Some(right) } => {
                let (left, right) = (self.facts(*left), self.facts(*right));
                Facts { holds: Narrowed::new(), fails: [left.fails, right.fails].concat() }
            }
            ExprKind::Infix { op, left, right: Some(right) } => {
                match COMPARISONS.iter().find(|row| row.token == *op) {
                    Some(row) => self.compared(row.holds, *left, *right),
                    None => Facts::default(),
                }
            }
            _ => Facts::default(),
        }
    }

    /// The facts of `left holds right`, for each operand an immutable integer binding.
    fn compared(&self, holds: vun_desugar::Comparison, left: ExprId, right: ExprId) -> Facts {
        let mut facts = Facts::default();
        let (Some(a), Some(b)) = (self.interval_of(left), self.interval_of(right)) else { return facts };
        for (operand, holds, own, other) in [(left, holds, &a, &b), (right, holds.flipped(), &b, &a)] {
            let Some(binder) = self.immutable(operand) else { continue };
            facts.holds.extend(holds.narrowed(own, other).map(|r| (binder, r)));
            facts.fails.extend(holds.negated().narrowed(own, other).map(|r| (binder, r)));
        }
        facts
    }

    /// The range of the integer `expr` has, typed already.
    fn interval_of(&self, expr: ExprId) -> Option<Interval> {
        self.found.types.get(&expr)?.ty.interval()
    }

    /// The immutable local `expr` names, if it names one.
    fn immutable(&self, expr: ExprId) -> Option<BinderId> {
        let ExprKind::Name(_) = self.hir().exprs[expr].kind else { return None };
        let Some(Res::Binder(binder)) = self.env().parsed.resolution.names[expr] else { return None };
        (!self.hir().binders[binder].mutable && self.locals.contains_key(&binder)).then_some(binder)
    }

    /// Narrows each binding of `facts` from here on, within what it is already.
    pub(super) fn narrow(&mut self, facts: &Narrowed) {
        for (binder, range) in facts {
            let current =
                self.narrowed.get(binder).copied().or_else(|| self.locals.get(binder).map(|k| k.ty));
            let Some(current) = current.and_then(|ty| ty.interval()) else { continue };
            if let Some(range) = current.meet(range) {
                self.narrowed.insert(*binder, TyKind::int(&range));
            }
        }
    }

    /// Runs `f` where `facts` hold, the narrowings in effect before restored after.
    pub(super) fn narrowing<R>(&mut self, facts: &Narrowed, f: impl FnOnce(&mut Self) -> R) -> R {
        let saved = self.narrowed.clone();
        self.narrow(facts);
        let result = f(self);
        self.narrowed = saved;
        result
    }

    /// After `stmt`, an `if` whose one branch cannot fall through, the code that follows runs
    /// where the other branch's condition holds.
    pub(super) fn after(&mut self, stmt: &Stmt) {
        let Stmt::Expr(e) = stmt else { return };
        let hir = self.hir();
        let ExprKind::If { cond: vun_hir::tree::Cond::Expr(cond), then, otherwise } = &hir.exprs[*e].kind
        else {
            return;
        };
        let otherwise_leaves = match otherwise {
            Some(o) => matches!(&hir.exprs[*o].kind, ExprKind::Block(b) if self.leaves(*b)),
            None => false,
        };
        let facts = self.facts(*cond);
        match (self.leaves(*then), otherwise_leaves) {
            (true, false) => self.narrow(&facts.fails),
            (false, true) => self.narrow(&facts.holds),
            _ => {}
        }
    }

    /// Whether `block` never falls through: a statement or its tail leaves it.
    pub(super) fn leaves(&self, block: BlockId) -> bool {
        let hir = self.hir();
        let b = &hir.blocks[block];
        let leaving = |e: ExprId| {
            matches!(
                hir.exprs[e].kind,
                ExprKind::Return(_) | ExprKind::Break { .. } | ExprKind::Continue { .. }
            )
        };
        b.stmts.iter().any(|s| matches!(s, Entry::One(Stmt::Expr(e)) if leaving(*e)))
            || b.tail.is_some_and(leaving)
    }
}
