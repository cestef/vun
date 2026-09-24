//! Loops (spec 01, section 6): `for { }` and `for cond { }` repeat their body through a head
//! block; `break v` leaves the loop it names, or the innermost, with its value, `continue` goes
//! back to the head, and `return v` leaves the function. Code after them is unreachable, lowered
//! into a fresh block nothing enters.

use vun_grammar::ITERATION;
use vun_hir::tree::{BlockId as HirBlock, ExprId, ForHead};
use vun_intern::Symbol;
use vun_origin::OriginId;
use vun_sema::member::MemberValue;
use vun_sema::ty::{Ty, Value};

use super::pattern::Matched;
use super::{Builder, Unbuilt};
use crate::body::{BlockId, Local, Operand, Rvalue, TerminatorKind};
use crate::place::{Place, Projection};

/// A loop being lowered.
pub(super) struct Loop {
    label: Option<Symbol>,
    head: BlockId,
    exit: BlockId,
    /// Where `break` writes the loop's value.
    result: Local,
    /// The deferred statements in scope outside the loop, which leaving it keeps.
    depth: usize,
}

impl Builder<'_, '_, '_> {
    /// `for { body }`, `for cond { body }`, `for p = e { body }` or `for p in e { body }`; a loop
    /// whose head ends it has the value `()`.
    pub(super) fn repeat(
        &mut self,
        label: Option<Symbol>,
        head: &ForHead,
        body: HirBlock,
        origin: OriginId,
    ) -> Result<Operand, Unbuilt> {
        // `for p in e` makes its iterator once, before the loop.
        let iterator = match head {
            ForHead::In { iter, .. } => Some(self.iterator(*iter, origin)?),
            _ => None,
        };
        let result = self.result(origin);
        let (start, exit) = (self.fresh(origin), self.fresh(origin));
        self.terminate(TerminatorKind::Goto(start), origin);
        self.current = start;
        match head {
            ForHead::Forever => {}
            ForHead::While(cond) => {
                let done = self.fresh(origin);
                self.condition(cond, done, origin)?;
                let run = self.current;
                self.current = done;
                self.assign(Place::of(result), Rvalue::Use(Self::unit()), origin);
                self.terminate(TerminatorKind::Goto(exit), origin);
                self.current = run;
            }
            ForHead::In { pat, .. } => {
                let (frame, step) = iterator.ok_or(Unbuilt { at: origin })?;
                let &(.., item, _) = ITERATION.first().ok_or(Unbuilt { at: origin })?;
                let resumed =
                    Place::of(self.temp_local(Rvalue::Resume { frame: Place::of(frame), step }, origin));
                let done = self.fresh(origin);
                let yielded = Rvalue::IsVariant { place: resumed.clone(), variant: Symbol::intern(item) };
                let cond = self.temp(yielded, origin);
                self.guard(cond, done, origin);
                let never = self.fresh(origin);
                self.blocks[never].term.kind = TerminatorKind::Unreachable;
                let matched = Matched::copied(resumed.project(Projection::Payload(0)));
                self.test(*pat, &matched, never)?;
                let run = self.current;
                self.current = done;
                self.assign(Place::of(result), Rvalue::Use(Self::unit()), origin);
                self.terminate(TerminatorKind::Goto(exit), origin);
                self.current = run;
            }
        }
        self.loops.push(Loop { label, head: start, exit, result, depth: self.defers.len() });
        let lowered = self.block(body);
        self.loops.pop();
        lowered?;
        self.terminate(TerminatorKind::Goto(start), origin);
        self.current = exit;
        Ok(Operand::Place(Place::of(result)))
    }

    /// The iterator `for p in e` drives, in a new local, and the type resuming it gives: made
    /// by the `iter` of the witness the typer found.
    fn iterator(&mut self, iter: ExprId, origin: OriginId) -> Result<(Local, Ty), Unbuilt> {
        let unbuilt = Unbuilt { at: origin };
        let &(_, function, ..) = ITERATION.first().ok_or(unbuilt)?;
        let iteration = *self.typed().ok_or(unbuilt)?.iterations.get(&iter).ok_or(unbuilt)?;
        let make = self.env.cx.get::<MemberValue>(&(iteration.witness, Symbol::intern(function)));
        let make = make.map_err(|_| unbuilt)?;
        let value = self.operand(iter)?;
        let call = Rvalue::Call { callee: Operand::Const(make), args: vec![value] };
        Ok((self.temp_local(call, origin), iteration.step))
    }

    /// `break 'label value`.
    pub(super) fn leave(
        &mut self,
        label: Option<Symbol>,
        value: Option<ExprId>,
        origin: OriginId,
    ) -> Result<Operand, Unbuilt> {
        let value = value.map(|v| self.operand(v)).transpose()?.unwrap_or_else(Self::unit);
        let (exit, result, depth) = self.target(label, origin).map(|l| (l.exit, l.result, l.depth))?;
        self.assign(Place::of(result), Rvalue::Use(value), origin);
        self.unwind(depth, origin)?;
        Ok(self.jump(TerminatorKind::Goto(exit), origin))
    }

    /// `continue 'label`.
    pub(super) fn again(&mut self, label: Option<Symbol>, origin: OriginId) -> Result<Operand, Unbuilt> {
        let (head, depth) = self.target(label, origin).map(|l| (l.head, l.depth))?;
        self.unwind(depth, origin)?;
        Ok(self.jump(TerminatorKind::Goto(head), origin))
    }

    /// `return value`.
    pub(super) fn exit(&mut self, value: Option<ExprId>, origin: OriginId) -> Result<Operand, Unbuilt> {
        let value = value.map(|v| self.operand(v)).transpose()?.unwrap_or_else(Self::unit);
        self.assign(Place::of(self.ret), Rvalue::Use(value), origin);
        self.unwind(0, origin)?;
        Ok(self.jump(TerminatorKind::Return, origin))
    }

    /// The loop `label` names, or the innermost.
    fn target(&self, label: Option<Symbol>, origin: OriginId) -> Result<&Loop, Unbuilt> {
        let mut loops = self.loops.iter().rev();
        match label {
            Some(label) => loops.find(|l| l.label == Some(label)),
            None => loops.next(),
        }
        .ok_or(Unbuilt { at: origin })
    }

    /// Ends the current block with `kind`, and continues in an unreachable block.
    fn jump(&mut self, kind: TerminatorKind, origin: OriginId) -> Operand {
        self.terminate(kind, origin);
        self.current = self.fresh(origin);
        Self::unit()
    }

    /// `()`.
    pub(super) fn unit() -> Operand {
        Operand::Const(Value::Tuple(Vec::new()))
    }
}
