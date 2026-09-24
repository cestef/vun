//! `defer s;` (spec 05, section 3): `s` is lowered again at every exit of its block, the latest
//! first, behind a `Defer` terminator naming the linear locals it consumes, so that the place
//! states run it only on exits where they are still `init`.

use vun_hir::tree::ExprId;
use vun_index::Idx;
use vun_origin::OriginId;
use vun_sema::kinds::Kinds;

use super::{Builder, Unbuilt};
use crate::body::{BlockId, Local, Operand, TerminatorKind};

impl Builder<'_, '_, '_> {
    /// `defer s`: runs at the exits of the enclosing block from here on.
    pub(super) fn deferred(&mut self, statement: ExprId) -> Operand {
        self.defers.push(statement);
        Self::unit()
    }

    /// Runs the deferred statements pushed since `depth`, the latest first, at an exit at
    /// `origin`.
    pub(super) fn unwind(&mut self, depth: usize, origin: OriginId) -> Result<(), Unbuilt> {
        for at in (depth..self.defers.len()).rev() {
            // An exit inside the statement runs only the statements deferred before it.
            let later = self.defers.split_off(at);
            let lowered = self.run_deferred(later[0], origin);
            self.defers.extend(later);
            lowered?;
        }
        Ok(())
    }

    fn run_deferred(&mut self, statement: ExprId, origin: OriginId) -> Result<(), Unbuilt> {
        let (run, skip) = (self.fresh(origin), self.fresh(origin));
        let (exit, before) = (self.current, self.locals.len());
        self.terminate(TerminatorKind::Defer { consumes: Vec::new(), run, skip }, origin);
        self.current = run;
        self.operand(statement)?;
        self.terminate(TerminatorKind::Goto(skip), origin);
        self.current = skip;
        let consumes = self.consumed(run, before);
        if let TerminatorKind::Defer { consumes: slot, .. } = &mut self.blocks[exit].term.kind {
            *slot = consumes;
        }
        Ok(())
    }

    /// The linear locals declared before `before` that the blocks from `first` on read whole,
    /// which moves them.
    fn consumed(&self, first: BlockId, before: usize) -> Vec<Local> {
        let mut consumed: Vec<Local> = self
            .blocks
            .iter()
            .filter(|(id, _)| id.index() >= first.index())
            .flat_map(|(_, b)| &b.stmts)
            .flat_map(|stmt| stmt.value.operands())
            .filter_map(|o| match o {
                Operand::Place(p) if p.projection.is_empty() && p.local.index() < before => Some(p.local),
                _ => None,
            })
            .filter(|&l| self.locals[l].ty.is_some_and(|t| self.env.cx.get::<Kinds>(&t).is_linear()))
            .collect();
        consumed.sort_unstable();
        consumed.dedup();
        consumed
    }
}
