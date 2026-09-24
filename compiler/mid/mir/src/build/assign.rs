//! Assignments to places: `p = e` stores `e`; `x op= e` stores `x op e` brought back to the type
//! of `x` by the narrowing the table `compound` gives (appendix B, spec 04 rule 9), the type
//! read from the typed body.

use overt_grammar::{COMPOUND, TokenKind};
use overt_hir::tree::ExprId;
use overt_origin::OriginId;

use super::{Builder, Unbuilt};
use crate::body::{Operand, Rvalue};

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
        let value = self.operand(value)?;
        if op == TokenKind::Eq {
            self.assign(target, Rvalue::Use(value), origin);
            return Ok(());
        }
        let (_, binary, narrowing) = COMPOUND.iter().find(|(token, ..)| *token == op).ok_or(unbuilt)?;
        let typed = self.typed().ok_or(unbuilt)?;
        let ty = typed.types.get(&place).ok_or(unbuilt)?.ty;
        let left = Operand::Place(target.clone());
        let operand = self.temp(Rvalue::Binary { op: *binary, left, right: value }, origin);
        self.assign(target, Rvalue::Narrow { operand, ty, narrowing: *narrowing }, origin);
        Ok(())
    }
}
