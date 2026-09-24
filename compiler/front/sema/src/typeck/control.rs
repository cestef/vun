//! Conditions and branches: `and`, `or`, `!` and `if` heads take the condition type of `lang`
//! (the table `forms`), and an `if` has the type both its branches can take.

use overt_hir::tree::{BlockId, Cond, ExprId, Hir};
use overt_hir::visit;

use super::{Known, Stage, Typer};
use crate::ty::TyKind;

impl Typer<'_, '_, '_> {
    /// Operands that must be conditions, and their result, a condition.
    pub(super) fn logic(&mut self, operands: &[ExprId]) -> Option<Known> {
        let condition = self.env.condition().ok();
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

    /// `if cond { then } else { otherwise }`: without `else`, its value is `()`.
    pub(super) fn branch(
        &mut self,
        hir: &Hir,
        cond: &Cond,
        then: BlockId,
        otherwise: Option<ExprId>,
    ) -> Option<Known> {
        let head = match cond {
            Cond::Expr(c) => self.logic(&[*c]).map(|k| k.stage),
            Cond::Test { .. } => {
                visit::walk_cond(self, hir, cond);
                None
            }
        };
        let then = self.block_value(then);
        let Some(otherwise) = otherwise else {
            return Some(Known { ty: TyKind::unit(), stage: Stage::Compile });
        };
        let found = self.typed(otherwise);
        let (then, found) = (then?, found?);
        let stage = then.stage.max(found.stage).max(head.unwrap_or(self.stage));
        let Some(ty) = then.ty.join(&found.ty) else {
            self.expect(hir.exprs[otherwise].origin, found, then.ty, None);
            return None;
        };
        Some(Known { ty, stage })
    }
}
