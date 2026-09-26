//! The accesses each statement makes, checked against the loans live there, and the loans the
//! result carries out at a return.

use vun_desugar::Access;
use vun_diag::{Msg, errors};
use vun_index::BitSet;
use vun_mir::{BlockId, Operand, Place, Rvalue, Stmt, TerminatorKind};
use vun_origin::OriginId;

use super::flow::{Holds, Live};
use super::{Checker, LoanId, Source};

/// A point of a block, with the loans each local holds there and the locals live there.
struct Here<'h> {
    block: BlockId,
    index: usize,
    holds: &'h Holds,
    live: &'h BitSet<vun_mir::Local>,
}

impl Checker<'_> {
    /// Checks the statements of `block` from the loans held at its entry.
    pub(super) fn check(&mut self, block: BlockId, mut holds: Holds, live: &Live) {
        let body = self.body;
        let b = &body.blocks[block];
        for (index, stmt) in b.stmts.iter().enumerate() {
            let own = self.taken.get(&(block, index)).copied();
            let loans = Self::live_loans(&holds, &live[block][index], own);
            if stmt.value == Rvalue::Dead {
                self.outlived(&loans, stmt.place.local, stmt.origin);
                self.hold(block, index, stmt, &mut holds);
                continue;
            }
            let here = Here { block, index, holds: &holds, live: &live[block][index] };
            for (place, access) in Self::accesses(stmt) {
                self.conflicts(&loans, place, access, stmt.origin, &here);
            }
            // Writing a place replaces its own storage: a borrow reached through a `.*` inside
            // it, such as the target of a borrow being reassigned, is not touched.
            let overwritten: BitSet<LoanId> =
                loans.iter().filter(|&l| !self.loans[l].place.beyond(&stmt.place)).collect();
            self.conflicts(&overwritten, &stmt.place, Access::Exclusive, stmt.origin, &here);
            self.hold(block, index, stmt, &mut holds);
        }
        let loans = Self::live_loans(&holds, &live[block][b.stmts.len()], None);
        let here = Here { block, index: b.stmts.len(), holds: &holds, live: &live[block][b.stmts.len()] };
        for operand in b.term.kind.operands() {
            if let Operand::Place(place) = operand {
                self.conflicts(&loans, place, Access::Shared, b.term.origin, &here);
            }
        }
        if b.term.kind == TerminatorKind::Return {
            self.escapes(&holds.iter_row(body.ret).collect(), b.term.origin);
        }
    }

    /// The loans some live local holds, but `own`, the one being taken.
    fn live_loans(holds: &Holds, live: &BitSet<vun_mir::Local>, own: Option<LoanId>) -> BitSet<LoanId> {
        let mut loans: BitSet<LoanId> = live.iter().flat_map(|local| holds.iter_row(local)).collect();
        if let Some(own) = own {
            loans.remove(own);
        }
        loans
    }

    /// The places `stmt` reaches before writing its place, and how: operands and tests read,
    /// borrows access as their mode says, resuming a frame is exclusive.
    pub(super) fn accesses(stmt: &Stmt) -> Vec<(&Place, Access)> {
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
            Rvalue::Ref { mode, place } => accesses.push((place, mode.access())),
            Rvalue::IsVariant { place, .. } => accesses.push((place, Access::Shared)),
            Rvalue::Resume { frame, .. } | Rvalue::End { frame } => accesses.push((frame, Access::Exclusive)),
            _ => {}
        }
        accesses
    }

    /// An access to `place` conflicts with a live loan of an overlapping place when either is
    /// exclusive.
    fn conflicts(
        &mut self,
        loans: &BitSet<LoanId>,
        place: &Place,
        access: Access,
        origin: OriginId,
        here: &Here<'_>,
    ) {
        for loan in loans.iter() {
            let l = &self.loans[loan];
            if l.source == Source::Taken
                && l.place.overlaps(place)
                && (l.mode.access() == Access::Exclusive || access == Access::Exclusive)
            {
                let what = self.name(&l.place);
                let msg = what.text(" is reached here while a borrow of it is still used later");
                let help = if l.mode.access() == Access::Exclusive {
                    Msg::new().text("an exclusive borrow is the only way to the place while it is used; finish with it before this")
                } else {
                    Msg::new().text("a shared borrow keeps the place from changing while it is used; write it after the borrow's last use")
                };
                let holders = here.live.iter().filter(|&l| here.holds.contains(l, loan)).collect();
                let later = self.later_use(here.block, here.index, &holders);
                if let Some(d) = self.reporting(errors::BORROW_CONFLICT, origin, msg, Some(loan), Some(help))
                {
                    let label = Msg::new().text("the borrow is used later here");
                    self.diag.error(match later {
                        Some(at) => d.label(at, Some(label)),
                        None => d,
                    });
                }
            }
        }
    }

    /// The loans of the function's own storage in its result, and those its `&temp` parameters
    /// lend.
    fn escapes(&mut self, loans: &BitSet<LoanId>, returned: OriginId) {
        for loan in loans.iter() {
            if self.loans[loan].source == Source::Lent {
                let msg = self
                    .name(&self.loans[loan].place)
                    .text(" is lent for the call only, and a borrow tied to it is returned");
                let help = Msg::new()
                    .text("a ")
                    .code("&temp")
                    .text(" parameter never reaches the result; return what another parameter lends");
                self.report(errors::BORROW_ESCAPE, returned, msg, Some(loan), Some(help));
            } else if !self.loans[loan].place.is_indirect()
                && !self.body.locals[self.loans[loan].place.local].role.is_static()
            {
                let origin = self.loans[loan].origin;
                let msg = self
                    .name(&self.loans[loan].place)
                    .text(" is local to this function, and a borrow of it is returned");
                let help =
                    Msg::new().text("return the value itself, or a borrow of something a parameter lends");
                self.report(errors::BORROW_ESCAPE, origin, msg, None, Some(help));
            }
        }
    }

    /// The scope of the binding in `local` ends at `origin`: a live loan of its own storage
    /// would outlive it (spec 05, section 4).
    fn outlived(&mut self, loans: &BitSet<LoanId>, local: vun_mir::Local, origin: OriginId) {
        for loan in loans.iter() {
            let l = &self.loans[loan];
            if l.source == Source::Taken && l.place.local == local && !l.place.is_indirect() {
                let msg =
                    self.name(&l.place).text(" goes out of scope here while a borrow of it is still used");
                let help = Msg::new().text("declare it in the scope where the borrow is used");
                self.report(errors::BORROW_ESCAPE, origin, msg, Some(loan), Some(help));
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
