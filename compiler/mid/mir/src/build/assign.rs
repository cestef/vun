//! Assignments to places: `p = e` stores `e`; `x op= e` stores `x op e` brought back to the type
//! of `x` by the narrowing the table `compound` gives (appendix B, spec 04 rule 9), the type
//! read from the typed body; `_ = e` discards `e`.

use vun_desugar::COMPOUND;
use vun_grammar::TokenKind;
use vun_hir::tree::{ExprId, ExprKind};
use vun_origin::OriginId;

use vun_sema::typeck::Operation;

use super::operator::Side;
use super::{Builder, Unbuilt};
use crate::Rvalue;

impl Builder<'_, '_, '_> {
    pub(super) fn store(
        &mut self,
        op: TokenKind,
        place: ExprId,
        value: ExprId,
        origin: OriginId,
    ) -> Result<(), Unbuilt> {
        let unbuilt = Unbuilt::at(origin);
        // `_ = e` evaluates `e` and discards it, into a temporary nothing consumes: a linear
        // value discarded is a leak the place checks find (spec 05, section 2).
        if op == TokenKind::Eq && matches!(self.env.parsed.hir.exprs[place].kind, ExprKind::Hole) {
            let value = self.operand(value)?;
            self.temp(Rvalue::Use(value), origin);
            return Ok(());
        }
        let target = self.place(place)?;
        let Some(narrowing) = COMPOUND.iter().find(|row| row.token == op).map(|row| row.narrowing) else {
            let value = self.operand(value)?;
            self.assign(target, Rvalue::Use(value), origin);
            return Ok(());
        };
        // `x op= y` is `x = (x op y)` narrowed back to the type of `x` (spec 04, rule 9).
        let typed = self.typed().ok_or(unbuilt)?;
        let ty = typed.types.get(&place).ok_or(unbuilt)?.ty;
        let operand = self.operator(Operation::Assign(place), place, vec![Side::Expr(value)], origin)?;
        self.assign(target, Rvalue::Narrow { operand, ty, narrowing }, origin);
        Ok(())
    }
}
