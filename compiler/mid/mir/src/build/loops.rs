//! Loops (spec 01, section 6): `for { }` and `for cond { }` repeat their body through a head
//! block; `break v` leaves the loop it names, or the innermost, with its value, `continue` goes
//! back to the head, and `return v` leaves the function. Code after them is unreachable, lowered
//! into a fresh block nothing enters.

use overt_hir::tree::{BlockId as HirBlock, Cond, ExprId, ForHead};
use overt_intern::Symbol;
use overt_origin::OriginId;
use overt_sema::ty::Value;

use super::{Builder, Unbuilt};
use crate::body::{BlockId, Operand, Rvalue, TerminatorKind};
use crate::place::Place;

/// A loop being lowered.
pub(super) struct Loop {
    label: Option<Symbol>,
    head: BlockId,
    exit: BlockId,
    /// Where `break` writes the loop's value.
    result: Place,
}

impl Builder<'_, '_, '_> {
    /// `for { body }` or `for cond { body }`; a loop whose condition fails has the value `()`.
    pub(super) fn repeat(
        &mut self,
        label: Option<Symbol>,
        head: &ForHead,
        body: HirBlock,
        origin: OriginId,
    ) -> Result<Operand, Unbuilt> {
        let result = self.result(origin);
        let (start, exit) = (self.fresh(origin), self.fresh(origin));
        self.terminate(TerminatorKind::Goto(start), origin);
        self.current = start;
        match head {
            ForHead::Forever => {}
            ForHead::While(Cond::Expr(cond)) => {
                let cond = self.operand(*cond)?;
                let (run, done) = (self.fresh(origin), self.fresh(origin));
                self.terminate(TerminatorKind::Branch { cond, then: run, otherwise: done }, origin);
                self.current = done;
                self.assign(result, Rvalue::Use(Self::unit()), origin);
                self.terminate(TerminatorKind::Goto(exit), origin);
                self.current = run;
            }
            ForHead::While(Cond::Test { .. }) | ForHead::In { .. } => return Err(Unbuilt { at: origin }),
        }
        self.loops.push(Loop { label, head: start, exit, result });
        let lowered = self.block(body);
        self.loops.pop();
        lowered?;
        self.terminate(TerminatorKind::Goto(start), origin);
        self.current = exit;
        Ok(Operand::Place(result))
    }

    /// `break 'label value`.
    pub(super) fn leave(
        &mut self,
        label: Option<Symbol>,
        value: Option<ExprId>,
        origin: OriginId,
    ) -> Result<Operand, Unbuilt> {
        let value = value.map(|v| self.operand(v)).transpose()?.unwrap_or_else(Self::unit);
        let (exit, result) = self.target(label, origin).map(|l| (l.exit, l.result))?;
        self.assign(result, Rvalue::Use(value), origin);
        Ok(self.jump(TerminatorKind::Goto(exit), origin))
    }

    /// `continue 'label`.
    pub(super) fn again(&mut self, label: Option<Symbol>, origin: OriginId) -> Result<Operand, Unbuilt> {
        let head = self.target(label, origin)?.head;
        Ok(self.jump(TerminatorKind::Goto(head), origin))
    }

    /// `return value`.
    pub(super) fn exit(&mut self, value: Option<ExprId>, origin: OriginId) -> Result<Operand, Unbuilt> {
        let value = value.map(|v| self.operand(v)).transpose()?.unwrap_or_else(Self::unit);
        self.assign(Place { local: self.ret }, Rvalue::Use(value), origin);
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
