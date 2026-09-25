//! `e?` (appendix B): the witness's `branch` of `lang.ops.Try` the typer found, then the value
//! the variant `go` carries, or a return of what `stop` carries (the table `early_return`).

use vun_desugar::EARLY_RETURN;
use vun_hir::tree::ExprId;
use vun_origin::OriginId;
use vun_sema::member::MemberValue;

use super::defer::Exit;
use super::{Builder, Unbuilt};
use crate::place::{Place, Projection};
use crate::{Operand, Role, Rvalue, TerminatorKind};

impl Builder<'_, '_, '_> {
    pub(super) fn early_return(
        &mut self,
        expr: ExprId,
        operand: ExprId,
        origin: OriginId,
    ) -> Result<Operand, Unbuilt> {
        let unbuilt = Unbuilt::at(origin);
        // The variant `stop` is the other one, so a failed test of `go` is a stop.
        let row = EARLY_RETURN.first().ok_or(unbuilt)?;
        let typed = self.typed().ok_or(unbuilt)?;
        let method = typed.methods.get(&expr).ok_or(unbuilt)?;
        let branch = self.env.cx.get::<MemberValue>(method).map_err(|_| unbuilt)?;
        let value = self.operand(operand)?;
        let call = Rvalue::Call { callee: Operand::Const(branch), args: vec![value] };
        // Its parts carry what it held: the value going on, or what is returned.
        let branched = self.temp_local(call, origin);
        self.locals[branched].role = Role::Unpacked;
        let branched = Place::of(branched);
        let stopped = self.fresh(origin);
        let going =
            self.temp(Rvalue::IsVariant { place: branched.clone(), variant: row.go.symbol() }, origin);
        self.guard(going, stopped, origin);
        let carried = |p: &Place, variant: vun_items::Item| {
            Rvalue::Use(Operand::Place(
                p.clone().project(Projection::Payload { variant: variant.symbol(), at: 0 }),
            ))
        };
        let result = self.temp(carried(&branched, row.go), origin);
        let on = self.current;
        self.current = stopped;
        self.assign(Place::of(self.ret), carried(&branched, row.stop), origin);
        self.unwind(0, origin, Exit::Function)?;
        self.terminate(TerminatorKind::Return, origin);
        self.current = on;
        Ok(result)
    }
}
