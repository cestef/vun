//! `e?` (appendix B): the witness's `branch` of `lang.ops.Try` the typer found, then the value
//! the variant `go` carries, or a return of what `stop` carries (the table `early_return`).

use overt_grammar::EARLY_RETURN;
use overt_hir::tree::ExprId;
use overt_intern::Symbol;
use overt_origin::OriginId;
use overt_sema::member::MemberValue;

use super::{Builder, Unbuilt};
use crate::body::{Operand, Rvalue, TerminatorKind};
use crate::place::{Place, Projection};

impl Builder<'_, '_, '_> {
    pub(super) fn early_return(
        &mut self,
        expr: ExprId,
        operand: ExprId,
        origin: OriginId,
    ) -> Result<Operand, Unbuilt> {
        let unbuilt = Unbuilt { at: origin };
        // The variant `stop` is the other one, so a failed test of `go` is a stop.
        let &(_, _, go, _) = EARLY_RETURN.first().ok_or(unbuilt)?;
        let typed = self.typed().ok_or(unbuilt)?;
        let method = typed.methods.get(&expr).ok_or(unbuilt)?;
        let branch = self.env.cx.get::<MemberValue>(method).map_err(|_| unbuilt)?;
        let value = self.operand(operand)?;
        let call = Rvalue::Call { callee: Operand::Const(branch), args: vec![value] };
        let branched = Place::of(self.temp_local(call, origin));
        let stopped = self.fresh(origin);
        let going =
            self.temp(Rvalue::IsVariant { place: branched.clone(), variant: Symbol::intern(go) }, origin);
        self.guard(going, stopped, origin);
        let carried = |p: &Place| Rvalue::Use(Operand::Place(p.clone().project(Projection::Payload(0))));
        let result = self.temp(carried(&branched), origin);
        let on = self.current;
        self.current = stopped;
        self.assign(Place::of(self.ret), carried(&branched), origin);
        self.unwind(0, origin)?;
        self.terminate(TerminatorKind::Return, origin);
        self.current = on;
        Ok(result)
    }
}
