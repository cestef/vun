//! `e?` (appendix B): `match lang.ops.Try.branch(e) { .go(v) => v, .stop(r) => return r }`,
//! through the table `early_return`. Its value has the type `go` carries, and what `stop` carries
//! must be accepted by the function's result type.

use overt_grammar::EARLY_RETURN;
use overt_hir::tree::ExprId;
use overt_intern::Symbol;

use super::{Known, Typer};
use crate::member::{Found, MemberType, Members};
use crate::ty::Ty;

impl Typer<'_, '_, '_> {
    pub(super) fn early_return(&mut self, expr: ExprId, operand: ExprId) -> Option<Known> {
        let known = self.typed(operand)?;
        let &(interface, function, go, stop) = EARLY_RETURN.first()?;
        let witness = self.witness_of(interface, known.ty)?;
        let function = Symbol::intern(function);
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
    fn payload(&self, ty: Ty, variant: &str) -> Option<Ty> {
        match self.env().cx.get::<Members>(&ty)?.find(Symbol::intern(variant)) {
            Found::Variant(v) => v.payload.first()?.as_ref().ok().copied(),
            _ => None,
        }
    }
}
