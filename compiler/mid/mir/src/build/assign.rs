//! Assignments to places: `p = e` stores `e`; `x op= e` stores `x op e` brought back to the type
//! of `x` by the narrowing the table `compound` gives (appendix B, spec 04 rule 9), the type
//! read from the typed body.

use vun_desugar::COMPOUND;
use vun_grammar::TokenKind;
use vun_hir::tree::ExprId;
use vun_origin::OriginId;

use vun_sema::typeck::Operation;

use super::operator::Side;
use super::{Builder, Unbuilt};
use crate::body::Rvalue;

impl Builder<'_, '_, '_> {
    pub(super) fn store(
        &mut self,
        op: TokenKind,
        place: ExprId,
        value: ExprId,
        origin: OriginId,
    ) -> Result<(), Unbuilt> {
        let unbuilt = Unbuilt { at: origin };
        let target = self.place(place)?;
        let Some(&(_, _, narrowing)) = COMPOUND.iter().find(|(token, ..)| *token == op) else {
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
