//! The accesses each statement makes, checked against the loans live there, and the loans the
//! result carries out at a return.

use std::collections::BTreeSet;

use overt_diag::{Msg, errors};
use overt_grammar::Access;
use overt_mir::{BlockId, Operand, Place, Rvalue, Stmt, TerminatorKind};
use overt_origin::OriginId;

use super::flow::{Holds, Live};
use super::{Checker, LoanId};

impl Checker<'_> {
    /// Checks the statements of `block` from the loans held at its entry.
    pub(super) fn check(&mut self, block: BlockId, mut holds: Holds, live: &Live) {
        let body = self.body;
        let b = &body.blocks[block];
        for (index, stmt) in b.stmts.iter().enumerate() {
            let own = self.taken.get(&(block, index)).copied();
            let loans = Self::live_loans(&holds, &live[block][index], own);
            for (place, access) in Self::accesses(stmt) {
                self.conflicts(&loans, place, access, stmt.origin);
            }
            self.hold(block, index, stmt, &mut holds);
        }
        let loans = Self::live_loans(&holds, &live[block][b.stmts.len()], None);
        for operand in b.term.kind.operands() {
            if let Operand::Place(place) = operand {
                self.conflicts(&loans, place, Access::Shared, b.term.origin);
            }
        }
        if b.term.kind == TerminatorKind::Return {
            self.escapes(&holds[body.ret]);
        }
    }

    /// The loans some live local holds, but `own`, the one being taken.
    fn live_loans(holds: &Holds, live: &BTreeSet<overt_mir::Local>, own: Option<LoanId>) -> BTreeSet<LoanId> {
        live.iter().flat_map(|&l| holds[l].iter().copied()).filter(|&loan| Some(loan) != own).collect()
    }

    /// The places `stmt` reaches and how: operands and tests read, borrows access as their mode
    /// says, resuming a frame and writing are exclusive.
    fn accesses(stmt: &Stmt) -> Vec<(&Place, Access)> {
        let mut accesses: Vec<(&Place, Access)> = stmt
            .value
            .operands()
            .into_iter()
            .filter_map(|o| match o {
                Operand::Place(p) => Some((p, Access::Shared)),
                Operand::Const(_) => None,
            })
            .collect();
        match &stmt.value {
            Rvalue::Ref { mode, place } => accesses.push((place, Self::access(*mode))),
            Rvalue::IsVariant { place, .. } => accesses.push((place, Access::Shared)),
            Rvalue::Resume { frame, .. } => accesses.push((frame, Access::Exclusive)),
            _ => {}
        }
        accesses.push((&stmt.place, Access::Exclusive));
        accesses
    }

    /// An access to `place` conflicts with a live loan of an overlapping place when either is
    /// exclusive.
    fn conflicts(&mut self, loans: &BTreeSet<LoanId>, place: &Place, access: Access, origin: OriginId) {
        for &loan in loans {
            let l = &self.loans[loan];
            if l.place.overlaps(place) && (l.access == Access::Exclusive || access == Access::Exclusive) {
                let what = self.name(&l.place);
                let msg = what.text(" is reached here while a borrow of it is still used later");
                self.report(errors::BORROW_CONFLICT, origin, msg, Some(loan));
            }
        }
    }

    /// The loans of the function's own storage in its result.
    fn escapes(&mut self, loans: &BTreeSet<LoanId>) {
        for &loan in loans {
            if !self.loans[loan].place.is_indirect() {
                let origin = self.loans[loan].origin;
                let msg = self
                    .name(&self.loans[loan].place)
                    .text(" is local to this function, and a borrow of it is returned");
                self.report(errors::BORROW_ESCAPE, origin, msg, None);
            }
        }
    }

    /// The local a place is in, by name where it has one.
    fn name(&self, place: &Place) -> Msg {
        self.body.locals[place.local]
            .name
            .map_or_else(|| Msg::new().text("this value"), |n| Msg::new().code(n.as_str()))
    }
}
