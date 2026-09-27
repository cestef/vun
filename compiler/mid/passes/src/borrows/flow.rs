//! The two flows the rules read: which loans each local may hold, forward from the entry, and
//! which locals are live, backward from each use.

use std::collections::VecDeque;

use vun_desugar::Extent;
use vun_index::{BitMatrix, BitSet, IndexVec};
use vun_mir::{BlockId, Local, Operand, Rvalue, Stmt, Terminator, TerminatorKind};

use super::{Checker, LoanId};
use crate::flow::Forward;

/// The loans each local may hold.
pub(super) type Holds = BitMatrix<Local, LoanId>;

/// The locals live before each statement of a block, and before its terminator last.
pub(super) type Live = IndexVec<BlockId, Vec<BitSet<Local>>>;

impl Checker<'_> {
    /// The loans each local may hold at the entry of each block reached.
    pub(super) fn holding(&self) -> IndexVec<BlockId, Option<Holds>> {
        let mut entry = Holds::new(self.body.locals.len(), self.loans.len());
        for &(param, loan) in &self.lent {
            entry.insert(param, loan);
        }
        let mut this = self;
        this.solve(self.body.blocks.len(), entry)
    }

    /// What `stmt` makes its place hold: the loan it takes, with those of the borrow it goes
    /// through; or, for a scoped value, the loans of everything it is built from but the
    /// arguments a call holds for itself only (`&temp`).
    pub(super) fn hold(&self, block: BlockId, index: usize, stmt: &Stmt, holds: &mut Holds) {
        let flows: BitSet<LoanId> = match &stmt.value {
            Rvalue::Ref { place, .. } => {
                let mut flows: BitSet<LoanId> =
                    self.taken.get(&(block, index)).copied().into_iter().collect();
                if place.is_indirect() {
                    flows.extend(holds.iter_row(place.local));
                }
                flows
            }
            value if self.locals.scoped.contains(&stmt.place.local) => {
                let temp = self.calls.get(&(block, index));
                Self::built_from(value, temp)
                    .into_iter()
                    .filter_map(|o| match o {
                        Operand::Place(p) => Some(p.local),
                        Operand::Const(_) => None,
                    })
                    .flat_map(|l| holds.iter_row(l))
                    .collect()
            }
            _ => BitSet::new(),
        };
        if stmt.place.projection.is_empty() {
            holds.set_row(stmt.place.local, &flows);
        } else {
            holds.union_row(stmt.place.local, &flows);
        }
    }

    /// The operands a value is built from: for a call, the callee and the arguments but those
    /// its callee holds for the call only.
    fn built_from<'v>(value: &'v Rvalue, extents: Option<&Vec<Extent>>) -> Vec<&'v Operand> {
        match (value, extents) {
            (Rvalue::Call { callee, args }, Some(extents)) => std::iter::once(callee)
                .chain(
                    args.iter()
                        .enumerate()
                        .filter(|(i, _)| extents.get(*i) != Some(&Extent::Temp))
                        .map(|(_, a)| a),
                )
                .collect(),
            _ => value.operands(),
        }
    }

    /// The locals live before each statement and terminator: read later on some path before
    /// being written whole.
    pub(super) fn liveness(&self) -> Live {
        let blocks = &self.body.blocks;
        let mut predecessors: IndexVec<BlockId, Vec<BlockId>> = IndexVec::filled(Vec::new(), blocks.len());
        for (block, b) in blocks.iter() {
            for next in b.term.kind.successors() {
                predecessors[next].push(block);
            }
        }
        let mut entry: IndexVec<BlockId, BitSet<Local>> = IndexVec::filled(BitSet::new(), blocks.len());
        let mut work: VecDeque<BlockId> = blocks.iter().map(|(block, _)| block).collect();
        while let Some(block) = work.pop_front() {
            let live = self.live_in(block, &entry).swap_remove(0);
            if live != entry[block] {
                entry[block] = live;
                work.extend(predecessors[block].iter().copied());
            }
        }
        blocks.iter().map(|(block, _)| self.live_in(block, &entry)).collect::<Vec<_>>().into()
    }

    /// The locals live before each statement of `block` and its terminator, given those live at
    /// the entry of each block.
    fn live_in(&self, block: BlockId, entry: &IndexVec<BlockId, BitSet<Local>>) -> Vec<BitSet<Local>> {
        let b = &self.body.blocks[block];
        let mut live = BitSet::new();
        for next in b.term.kind.successors() {
            live.union(&entry[next]);
        }
        live.extend(self.terminator_uses(&b.term));
        let mut before = vec![live.clone()];
        for stmt in b.stmts.iter().rev() {
            if stmt.place.projection.is_empty() {
                live.remove(stmt.place.local);
            }
            live.extend(Self::uses(stmt));
            before.push(live.clone());
        }
        before.reverse();
        before
    }

    /// The locals `stmt` reads: its operands, the places it borrows or tests, their indices, and
    /// a place it writes through `.*`.
    fn uses(stmt: &Stmt) -> Vec<Local> {
        let mut uses: Vec<Local> = stmt.value.operands().into_iter().flat_map(Self::operand_locals).collect();
        if let Rvalue::Ref { place, .. }
        | Rvalue::IsVariant { place, .. }
        | Rvalue::Resume { frame: place, .. }
        | Rvalue::End { frame: place } = &stmt.value
        {
            uses.push(place.local);
            uses.extend(place.indices());
        }
        uses.extend(stmt.place.indices());
        if stmt.place.is_indirect() {
            uses.push(stmt.place.local);
        }
        uses
    }

    fn terminator_uses(&self, term: &Terminator) -> Vec<Local> {
        let mut uses: Vec<Local> = term.kind.operands().into_iter().flat_map(Self::operand_locals).collect();
        if term.kind == TerminatorKind::Return {
            uses.push(self.body.ret);
        }
        uses
    }

    fn operand_locals(operand: &Operand) -> Vec<Local> {
        match operand {
            Operand::Place(p) => std::iter::once(p.local).chain(p.indices()).collect(),
            Operand::Const(_) => Vec::new(),
        }
    }
}

/// The loans each local may hold, forward from the entry: each statement makes its place hold
/// what it takes or is built from.
impl Forward for &Checker<'_> {
    type Fact = Holds;

    fn transfer(&mut self, block: BlockId, holds: &mut Holds) -> Vec<BlockId> {
        for (index, stmt) in self.body.blocks[block].stmts.iter().enumerate() {
            self.hold(block, index, stmt, holds);
        }
        self.body.blocks[block].term.kind.successors().into_iter().collect()
    }
}
