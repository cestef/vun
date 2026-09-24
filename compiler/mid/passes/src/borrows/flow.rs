//! The two flows the rules read: which loans each local may hold, forward from the entry, and
//! which locals are live, backward from each use.

use std::collections::{BTreeSet, VecDeque};

use vun_desugar::Extent;
use vun_index::{Idx, IndexVec};
use vun_mir::{BlockId, Local, Operand, Rvalue, Stmt, Terminator, TerminatorKind};

use super::{Checker, LoanId};

/// The loans each local may hold.
pub(super) type Holds = IndexVec<Local, BTreeSet<LoanId>>;

/// The locals live before each statement of a block, and before its terminator last.
pub(super) type Live = IndexVec<BlockId, Vec<BTreeSet<Local>>>;

impl Checker<'_> {
    /// The loans each local may hold at the entry of each block reached.
    pub(super) fn holding(&self) -> IndexVec<BlockId, Option<Holds>> {
        let mut at: IndexVec<BlockId, Option<Holds>> = IndexVec::filled(None, self.body.blocks.len());
        let mut entry = IndexVec::filled(BTreeSet::new(), self.body.locals.len());
        for &(param, loan) in &self.lent {
            entry[param].insert(loan);
        }
        at[BlockId::new(0)] = Some(entry);
        let mut work = VecDeque::from([BlockId::new(0)]);
        while let Some(block) = work.pop_front() {
            let Some(mut holds) = at[block].clone() else { continue };
            for (index, stmt) in self.body.blocks[block].stmts.iter().enumerate() {
                self.hold(block, index, stmt, &mut holds);
            }
            for next in self.body.blocks[block].term.kind.successors() {
                let changed = match &mut at[next] {
                    Some(there) => {
                        let mut changed = false;
                        for (local, loans) in holds.iter() {
                            let before = there[local].len();
                            there[local].extend(loans.iter().copied());
                            changed |= there[local].len() != before;
                        }
                        changed
                    }
                    slot @ None => {
                        *slot = Some(holds.clone());
                        true
                    }
                };
                if changed {
                    work.push_back(next);
                }
            }
        }
        at
    }

    /// What `stmt` makes its place hold: the loan it takes, with those of the borrow it goes
    /// through; or, for a scoped value, the loans of everything it is built from but the
    /// arguments a call holds for itself only (`&temp`).
    pub(super) fn hold(&self, block: BlockId, index: usize, stmt: &Stmt, holds: &mut Holds) {
        let flows: BTreeSet<LoanId> = match &stmt.value {
            Rvalue::Ref { place, .. } => {
                let mut flows: BTreeSet<LoanId> =
                    self.taken.get(&(block, index)).copied().into_iter().collect();
                if place.is_indirect() {
                    flows.extend(holds[place.local].iter().copied());
                }
                flows
            }
            value if self.scoped.contains(&stmt.place.local) => {
                let temp = self.calls.get(&(block, index));
                Self::built_from(value, temp)
                    .into_iter()
                    .filter_map(|o| match o {
                        Operand::Place(p) => Some(p.local),
                        Operand::Const(_) => None,
                    })
                    .flat_map(|l| holds[l].iter().copied())
                    .collect()
            }
            _ => BTreeSet::new(),
        };
        if stmt.place.projection.is_empty() {
            holds[stmt.place.local] = flows;
        } else {
            holds[stmt.place.local].extend(flows);
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
        let mut entry: IndexVec<BlockId, BTreeSet<Local>> = IndexVec::filled(BTreeSet::new(), blocks.len());
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
    fn live_in(&self, block: BlockId, entry: &IndexVec<BlockId, BTreeSet<Local>>) -> Vec<BTreeSet<Local>> {
        let b = &self.body.blocks[block];
        let mut live: BTreeSet<Local> =
            b.term.kind.successors().into_iter().flat_map(|next| entry[next].iter().copied()).collect();
        live.extend(self.terminator_uses(&b.term));
        let mut before = vec![live.clone()];
        for stmt in b.stmts.iter().rev() {
            if stmt.place.projection.is_empty() {
                live.remove(&stmt.place.local);
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
