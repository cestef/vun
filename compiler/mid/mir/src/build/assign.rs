//! Assignments to locals: `x = e` stores `e`; `x op= e` stores `x op e` brought back to the type
//! of `x` by the narrowing the table `compound` gives (appendix B, spec 04 rule 9), the type
//! read from the typed body.

use overt_grammar::{COMPOUND, TokenKind};
use overt_hir::resolve::Res;
use overt_hir::tree::{ExprId, ExprKind};
use overt_origin::OriginId;
use overt_sema::typeck::Typed;

use super::{Builder, Unbuilt};
use crate::body::{Operand, Rvalue};
use crate::place::Place;

impl Builder<'_, '_, '_> {
    pub(super) fn store(
        &mut self,
        op: TokenKind,
        place: ExprId,
        value: ExprId,
        origin: OriginId,
    ) -> Result<(), Unbuilt> {
        let unbuilt = Unbuilt { at: origin };
        let (ExprKind::Name(_), Some(Res::Binder(b))) =
            (&self.env.parsed.hir.exprs[place].kind, self.env.parsed.resolution.names[place])
        else {
            return Err(unbuilt);
        };
        let place_local = Place { local: *self.bound.get(&b).ok_or(unbuilt)? };
        let value = self.operand(value)?;
        if op == TokenKind::Eq {
            self.assign(place_local, Rvalue::Use(value), origin);
            return Ok(());
        }
        let (_, binary, narrowing) = COMPOUND.iter().find(|(token, ..)| *token == op).ok_or(unbuilt)?;
        let typed = self.env.cx.get::<Typed>(&self.f).ok_or(unbuilt)?;
        let ty = typed.types.get(&place).ok_or(unbuilt)?.ty;
        let left = Operand::Place(place_local);
        let operand = self.temp(Rvalue::Binary { op: *binary, left, right: value }, origin);
        self.assign(place_local, Rvalue::Narrow { operand, ty, narrowing: *narrowing }, origin);
        Ok(())
    }
}
