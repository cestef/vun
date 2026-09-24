//! What runs at the exits of a block, the latest first. `defer s;` (spec 05, section 3): `s` is
//! lowered again at every exit of its block, behind a `Defer` terminator naming the linear locals
//! it consumes, so that the place states run it only on exits where they are still `init`. A
//! binding's scope ends there too, with `Rvalue::Dead`, where the place states find a linear
//! value it still holds (spec 05, section 2).

use vun_hir::tree::{BinderId, ExprId};
use vun_index::Idx;
use vun_origin::OriginId;
use vun_sema::kinds::Kinds;

use super::{Builder, Unbuilt};
use crate::body::{BlockId, Local, Operand, Rvalue, TerminatorKind};
use crate::place::Place;

/// Where an exit goes: on in the function, where the bindings whose scope it leaves end, or out
/// of it, where the return checks what is still held.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Exit {
    Scope,
    Function,
}

/// What runs at the exits of a block.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Pending {
    Defer(ExprId),
    /// The end of the scope of the binding in the local.
    Binding(Local),
}

impl Builder<'_, '_, '_> {
    /// `defer s`: runs at the exits of the enclosing block from here on.
    pub(super) fn deferred(&mut self, statement: ExprId) -> Operand {
        self.pending.push(Pending::Defer(statement));
        Self::unit()
    }

    /// A local for `binder`, whose scope ends at the exits of the enclosing block.
    pub(super) fn bind_scoped(&mut self, binder: BinderId) -> Local {
        let local = self.bind(binder);
        self.pending.push(Pending::Binding(local));
        local
    }

    /// Runs what was pushed since `depth`, the latest first, at an exit at `origin` going to
    /// `exit`.
    pub(super) fn unwind(&mut self, depth: usize, origin: OriginId, exit: Exit) -> Result<(), Unbuilt> {
        for at in (depth..self.pending.len()).rev() {
            match self.pending[at] {
                Pending::Binding(local) if exit == Exit::Scope => {
                    self.assign(Place::of(local), Rvalue::Dead, origin);
                }
                Pending::Binding(_) => {}
                Pending::Defer(statement) => {
                    // An exit inside the statement runs only what was pushed before it.
                    let later = self.pending.split_off(at);
                    let lowered = self.run_deferred(statement, origin);
                    self.pending.extend(later);
                    lowered?;
                }
            }
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
