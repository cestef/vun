//! Terminators: jumps to the blocks MIR's terminators name, returns through the result's
//! storage, and what never comes back.

use vun_lir::{Target, Terminator};
use vun_mir::{BlockId, TerminatorKind};

use super::FnLower;
use crate::repr::Repr;
use crate::unlowered::{Form, Unlowered};

impl FnLower<'_, '_, '_> {
    pub(super) fn term(&mut self, id: BlockId, kind: &TerminatorKind) -> Result<(), Unlowered> {
        let jump = |block: vun_lir::Block| Target { block, args: Vec::new() };
        if self.coroutine.is_some()
            && let Some(term) = self.suspension(id, kind)?
        {
            self.b.terminate(term);
            return Ok(());
        }
        let term = match kind {
            TerminatorKind::Return => self.ret()?,
            TerminatorKind::Unreachable => Terminator::Unreachable,
            TerminatorKind::Goto(to) => Terminator::Jump(jump(self.blocks[*to])),
            TerminatorKind::Branch { cond, then, otherwise } => {
                let holds = self.holds(cond)?;
                Terminator::Branch {
                    cond: holds,
                    then: jump(self.blocks[*then]),
                    otherwise: jump(self.blocks[*otherwise]),
                }
            }
            // The place states decide, statically, whether a deferred statement runs.
            TerminatorKind::Defer { consumes, run, .. } => {
                let next = match self.defers.defers.get(&id) {
                    Some(next) => *next,
                    None if consumes.is_empty() => *run,
                    None => return Err(Unlowered::unsupported(self.at, Form::Defer)),
                };
                Terminator::Jump(jump(self.blocks[next]))
            }
            TerminatorKind::Yield { .. } | TerminatorKind::Ended => {
                return Err(Unlowered::unsupported(self.at, Form::Frame));
            }
        };
        self.b.terminate(term);
        Ok(())
    }

    /// Returns the result local: a scalar as the value, one in memory through the address the
    /// caller passed.
    pub(super) fn ret(&mut self) -> Result<Terminator, Unlowered> {
        self.unguard()?;
        let ty = self.ty(self.body.ret)?;
        let Some(addr) = self.local(self.body.ret) else { return Ok(Terminator::Return(None)) };
        match (self.l.types.repr(ty, self.at)?, self.out) {
            (Repr::Scalar(s), _) => {
                Ok(Terminator::Return(Some(self.b.load(s, addr, vun_lir::Access::Plain, self.at))))
            }
            (Repr::Memory(layout), Some(out)) => {
                self.b.copy(out, addr, layout.size, self.at);
                Ok(Terminator::Return(None))
            }
            _ => Ok(Terminator::Return(None)),
        }
    }
}
