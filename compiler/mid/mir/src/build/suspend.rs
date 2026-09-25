//! Suspension (spec 06): `yield v` ends a block with a `Yield`; driving a frame resumes it until
//! it returns, once per item it yields. `yield ..e` drives `e`'s frame, yielding each item, and
//! has what it returns; `for p in e` drives the iterator `e`'s witness of `lang.ops.Iterate`
//! makes (the table `iteration`).

use vun_desugar::ITERATION;
use vun_grammar::TokenKind;
use vun_hir::tree::{ExprId, ExprKind};
use vun_origin::OriginId;
use vun_sema::ty::Ty;

use super::defer::{Exit, Pending};
use super::{Builder, Unbuilt};
use crate::place::{Place, Projection};
use crate::{Local, Operand, Role, Rvalue, TerminatorKind};

impl Builder<'_, '_, '_> {
    /// `yield value`, or `yield ..frame`.
    pub(super) fn yielded(&mut self, value: ExprId, origin: OriginId) -> Result<Operand, Unbuilt> {
        let unbuilt = Unbuilt::at(origin);
        if let ExprKind::Prefix { op: TokenKind::DotDot, operand, .. } = self.env.parsed.hir.exprs[value].kind
        {
            let step = self.type_of(operand).and_then(|t| self.env().resumed(t)).ok_or(unbuilt)?;
            let value = self.operand(operand)?;
            let frame = self.temp_local(Rvalue::Use(value), origin);
            // Ending this frame while it drives the other ends the other too.
            let depth = self.pending.len();
            self.pending.push(Pending::End(frame));
            let driven = self.drive(frame, step, origin, |b, item| b.suspend(Operand::Place(item), origin));
            self.pending.truncate(depth);
            return driven;
        }
        let value = self.operand(value)?;
        self.suspend(value, origin)?;
        Ok(Self::unit())
    }

    /// Suspends with `value`, going on in a new block when resumed; ending the frame here runs
    /// what is pending, the latest first.
    fn suspend(&mut self, value: Operand, origin: OriginId) -> Result<(), Unbuilt> {
        let (resume, cancel) = (self.fresh(origin), self.fresh(origin));
        self.terminate(TerminatorKind::Yield { value, resume, cancel }, origin);
        self.current = cancel;
        self.unwind(0, origin, Exit::Function)?;
        self.terminate(TerminatorKind::Ended, origin);
        self.current = resume;
        Ok(())
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
        let unbuilt = Unbuilt::at(origin);
        let row = ITERATION.first().ok_or(unbuilt)?;
        let head = self.fresh(origin);
        self.terminate(TerminatorKind::Goto(head), origin);
        self.current = head;
        // Each step is taken apart: the item it yields, or what it returns.
        let resumed = self.temp_local(Rvalue::Resume { frame: Place::of(frame), step }, origin);
        self.locals[resumed].role = Role::Unpacked;
        let resumed = Place::of(resumed);
        let returned = self.fresh(origin);
        let yielded = Rvalue::IsVariant { place: resumed.clone(), variant: row.each.symbol() };
        let cond = self.temp(yielded, origin);
        self.guard(cond, returned, origin);
        each(self, resumed.clone().project(Projection::Payload { variant: row.each.symbol(), at: 0 }))?;
        self.terminate(TerminatorKind::Goto(head), origin);
        self.current = returned;
        let done = Projection::Payload { variant: row.done.symbol(), at: 0 };
        let result = self.temp(Rvalue::Use(Operand::Place(resumed.project(done))), origin);
        // The frame returned; ending it frees it (spec 06, section 1).
        self.temp(Rvalue::End { frame: Place::of(frame) }, origin);
        Ok(result)
    }
}
