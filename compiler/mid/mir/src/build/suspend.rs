//! Suspension (spec 06): `yield v` ends a block with a `Yield`; driving a frame resumes it until
//! it returns, once per item it yields. `yield ..e` drives `e`'s frame, yielding each item, and
//! has what it returns; `for p in e` drives the iterator `e`'s witness of `lang.ops.Iterate`
//! makes (the table `iteration`).

use overt_grammar::{ITERATION, TokenKind};
use overt_hir::tree::{ExprId, ExprKind};
use overt_intern::Symbol;
use overt_origin::OriginId;
use overt_sema::ty::Ty;

use super::{Builder, Unbuilt};
use crate::body::{Local, Operand, Rvalue, TerminatorKind};
use crate::place::{Place, Projection};

impl Builder<'_, '_, '_> {
    /// `yield value`, or `yield ..frame`.
    pub(super) fn yielded(&mut self, value: ExprId, origin: OriginId) -> Result<Operand, Unbuilt> {
        let unbuilt = Unbuilt { at: origin };
        if let ExprKind::Prefix { op: TokenKind::DotDot, operand, .. } = self.env.parsed.hir.exprs[value].kind
        {
            let step = self.type_of(operand).and_then(|t| self.env().resumed(t)).ok_or(unbuilt)?;
            let value = self.operand(operand)?;
            let frame = self.temp_local(Rvalue::Use(value), origin);
            return self.drive(frame, step, origin, |b, item| {
                b.suspend(Operand::Place(item), origin);
                Ok(())
            });
        }
        let value = self.operand(value)?;
        self.suspend(value, origin);
        Ok(Self::unit())
    }

    /// Suspends with `value`, going on in a new block when resumed.
    fn suspend(&mut self, value: Operand, origin: OriginId) {
        let resume = self.fresh(origin);
        self.terminate(TerminatorKind::Yield { value, resume }, origin);
        self.current = resume;
    }

    /// Resumes the frame in `frame` until it returns: `each` runs for the place of every item it
    /// yields, and the result is what it returns. `step` is the type resuming gives.
    pub(super) fn drive(
        &mut self,
        frame: Local,
        step: Ty,
        origin: OriginId,
        mut each: impl FnMut(&mut Self, Place) -> Result<(), Unbuilt>,
    ) -> Result<Operand, Unbuilt> {
        let unbuilt = Unbuilt { at: origin };
        let &(.., item, _) = ITERATION.first().ok_or(unbuilt)?;
        let head = self.fresh(origin);
        self.terminate(TerminatorKind::Goto(head), origin);
        self.current = head;
        let resumed = Place::of(self.temp_local(Rvalue::Resume { frame: Place::of(frame), step }, origin));
        let returned = self.fresh(origin);
        let yielded = Rvalue::IsVariant { place: resumed.clone(), variant: Symbol::intern(item) };
        let cond = self.temp(yielded, origin);
        self.guard(cond, returned, origin);
        each(self, resumed.clone().project(Projection::Payload(0)))?;
        self.terminate(TerminatorKind::Goto(head), origin);
        self.current = returned;
        Ok(self.temp(Rvalue::Use(Operand::Place(resumed.project(Projection::Payload(0)))), origin))
    }
}
