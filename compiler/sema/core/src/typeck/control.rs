//! Conditions and branches: `and`, `or`, `!` and `if` heads take the condition type of `lang`
//! (the table `forms`), and an `if` has the type both its branches can take.

use vun_grammar::TokenKind;
use vun_hir::tree::{BlockId, Cond, ExprId};
use vun_sema_ty::{Ty, TyKind};

use super::{Known, Stage, Typer};

impl Typer<'_, '_, '_> {
    /// Operands that must be conditions, and their result, a condition.
    pub(super) fn logic(&mut self, operands: &[ExprId]) -> Option<Known> {
        let condition = self.env().condition().ok();
        let mut stage = Stage::Compile;
        for &operand in operands {
            let known = self.typed(operand);
            stage = stage.max(known.map_or(self.stage, |k| k.stage));
            if let (Some(known), Some(condition)) = (known, condition) {
                self.expect(self.hir().exprs[operand].origin, known, condition, None);
            }
        }
        Some(Known { ty: condition?, stage })
    }

    /// `left and right`, whose right operand is typed where `left` holds, or `left or right`,
    /// where it fails.
    pub(super) fn connective(&mut self, op: TokenKind, left: ExprId, right: ExprId) -> Option<Known> {
        let first = self.logic(&[left]);
        let facts = self.facts(left);
        let known = if op == TokenKind::And { facts.holds } else { facts.fails };
        let second = self.narrowing(&known, |t| t.logic(&[right]));
        Some(Known { stage: first?.stage.max(second?.stage), ..second? })
    }

    /// `if cond { then } else { otherwise }`: without `else`, its value is `()`.
    pub(super) fn branch(
        &mut self,
        expr: ExprId,
        cond: &Cond,
        then: BlockId,
        otherwise: Option<ExprId>,
    ) -> Option<Known> {
        let expected = self.expected(expr);
        let mut facts = super::narrow::Facts::default();
        let head = match cond {
            Cond::Expr(c) => {
                let head = self.logic(&[*c]).map(|k| k.stage);
                facts = self.facts(*c);
                head
            }
            // `if p = e`: `p` is matched against `e` as a `match` arm is.
            Cond::Test { pat, value } => {
                let matched = self.typed(*value);
                self.test(*pat, matched.map(|k| k.ty));
                matched.map(|k| k.stage)
            }
        };
        let then_value = self.narrowing(&facts.holds, |t| t.block_value(then, expected));
        let Some(otherwise) = otherwise else {
            return Some(Known { ty: TyKind::unit(), stage: Stage::Compile });
        };
        let then_at = self.hir().blocks[then].tail;
        let then = match then_at {
            Some(tail) => self.fits(tail, then_value, expected),
            None => then_value,
        };
        let found = self.narrowing(&facts.fails, |t| t.typed_as(otherwise, expected));
        let found = self.fits(otherwise, found, expected);
        let joined = self.join(otherwise, then?, found?)?;
        Some(Known { stage: joined.stage.max(head.unwrap_or(self.stage)), ..joined })
    }

    /// A branch's value `found` at `at`, reported there when it does not fit the type the
    /// context expects of the whole; `None` then, so that the whole is not reported again.
    pub(super) fn fits(&mut self, at: ExprId, found: Option<Known>, expected: Option<Ty>) -> Option<Known> {
        let (found, expected) = (found?, expected);
        match expected {
            Some(expected) if !expected.accepts(&found.ty) => {
                let written = self.written.filter(|_| self.result == Some(expected));
                self.expect(self.hir().exprs[at].origin, found, expected, written);
                None
            }
            _ => Some(found),
        }
    }

    /// The type of what never finishes, such as a `return`: the empty `never`, which fits any
    /// type expected of it.
    pub(super) fn never(&self) -> Option<Known> {
        let never = self.env().item(vun_items::lang::never).ok()?.as_type()?;
        Some(Known { ty: never, stage: Stage::Compile })
    }

    /// The type both a branch typed so far, `sofar`, and the branch `at`, `found`, can take;
    /// branches with none in common are reported at `at`.
    pub(super) fn join(&mut self, at: ExprId, sofar: Known, found: Known) -> Option<Known> {
        let Some(ty) = sofar.ty.join(&found.ty) else {
            self.expect(self.hir().exprs[at].origin, found, sofar.ty, None);
            return None;
        };
        Some(Known { ty, stage: sofar.stage.max(found.stage) })
    }
}
