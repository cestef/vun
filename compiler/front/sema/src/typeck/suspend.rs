//! `yield v` and `yield ..e` (spec 06): a value yielded is checked against what the coroutine
//! function yields; delegating yields what `e`'s frame yields and has what it returns.

use overt_grammar::TokenKind;
use overt_hir::tree::{ExprId, ExprKind};

use super::{Known, Stage, Typer};
use crate::ty::TyKind;

impl Typer<'_, '_, '_> {
    pub(super) fn yielded(&mut self, value: ExprId) -> Option<Known> {
        let hir = self.hir();
        if let ExprKind::Prefix { op: TokenKind::DotDot, operand, .. } = hir.exprs[value].kind {
            let delegated = self.typed(operand)?;
            let (yields, returns) = self.env().frame_step(delegated.ty)?;
            if let Some(expected) = self.yields
                && !expected.accepts(&yields)
            {
                self.expect(hir.exprs[operand].origin, Known { ty: yields, ..delegated }, expected, None);
            }
            return Some(Known { ty: returns, stage: self.stage });
        }
        let found = self.typed_as(value, self.yields)?;
        if let Some(expected) = self.yields {
            self.expect(hir.exprs[value].origin, found, expected, None);
        }
        Some(Known { ty: TyKind::unit(), stage: Stage::Compile })
    }
}
