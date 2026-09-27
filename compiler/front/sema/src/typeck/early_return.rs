//! `e?` (appendix B): `match lang.ops.Try.branch(e) { .go(v) => v, .stop(r) => return r }`,
//! through the table `early_return`. Its value has the type `go` carries, and what `stop` carries
//! must be accepted by the function's result type.

use vun_desugar::{EARLY_RETURN, EarlyReturnRow};
use vun_hir::tree::ExprId;
use vun_items::Item;

use super::{Known, Typer};
use crate::member::{Found, MemberType, Members};
use crate::ty::Ty;

impl Typer<'_, '_, '_> {
    pub(super) fn early_return(&mut self, expr: ExprId, operand: ExprId) -> Option<Known> {
        let known = self.typed(operand)?;
        let &EarlyReturnRow { function, go, stop } = EARLY_RETURN.first()?;
        let Some(witness) = self.witness_of(function.parent()?, known.ty) else {
            self.unimplemented(operand, function.parent(), known.ty);
            return None;
        };
        let function = function.symbol();
        let branch = self.env().cx.get::<MemberType>(&(witness, function)).ok()?.ty.signature()?.result;
        self.methods.insert(expr, (witness, function));
        let (value, stopped) = (self.payload(branch, go)?, self.payload(branch, stop)?);
        if let Some(result) = self.result
            && !result.accepts(&stopped)
        {
            let origin = self.hir().exprs[expr].origin;
            self.expect(origin, Known { ty: stopped, stage: known.stage }, result, self.written);
        }
        Some(Known { ty: value, stage: known.stage })
    }

    /// The type the variant `variant` of the enum type `ty` carries.
    fn payload(&self, ty: Ty, variant: Item) -> Option<Ty> {
        match self.env().cx.get::<Members>(&ty)?.find(variant.symbol()) {
            Found::Variant(v) => v.payload.first()?.as_ref().ok().copied(),
            _ => None,
        }
    }
}
