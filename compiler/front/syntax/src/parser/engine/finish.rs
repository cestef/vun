//! Ending frames: a reading completes as the kinds that may end where it ends, each with what the
//! positions its path took take for its children; an operand gives what it read to the reading
//! below it, whose positions that may hold it stay.

use vun_grammar::{NodeKind, Positions};
use vun_trace::tracing;

use super::frame::{Candidate, Content, Frame, Reading};
use super::spec::Spec;
use super::tree::Piece;
use crate::cst::Anchor;
use crate::parser::Parser;

impl Parser<'_, '_, '_> {
    /// Ends the top frame; returns the root once it completes.
    pub(super) fn end_top(&mut self) -> Option<u32> {
        match self.stack.pop().expect("a frame ends") {
            Frame::Reading(r) => {
                let n = self.complete(r);
                match self.stack.last_mut() {
                    Some(Frame::Operand(o)) => {
                        o.piece = Some(n);
                        None
                    }
                    _ => Some(n),
                }
            }
            Frame::Operand(o) => {
                self.depth -= 1;
                self.deliver(o.piece.expect("an operand ends once read"));
                None
            }
        }
    }

    /// Adds the node `r` read, spelling the kinds that may end where it ends, preferred first; an
    /// `Error` where it chained an operator without grouping.
    fn complete(&mut self, r: Reading) -> u32 {
        let outer = self.outer(self.stack.len()).copied();
        let ending: Vec<Candidate> = r.candidates.iter().copied().filter(|c| c.accepts()).collect();
        let ending = if ending.is_empty() { r.candidates.clone() } else { ending };
        if r.wrong {
            self.tree.spell(NodeKind::Error, []);
        }
        for &c in ending.iter().filter(|_| !r.wrong) {
            let specs = self.path_specs(&r, c, outer.as_ref());
            self.tree.spell(c.kind, specs);
        }
        let anchor = r.anchor.unwrap_or(Anchor::Missing(self.current()));
        tracing::trace!(kinds = ?ending.iter().map(|c| c.kind).collect::<Vec<_>>(), ?anchor, "read");
        self.tree.add(anchor, r.pieces)
    }

    /// What the positions a reading of `c` took take for each child it read, in order: from the
    /// positions it ends at, back through those leading to them.
    fn path_specs(&self, r: &Reading, c: Candidate, outer: Option<&Spec>) -> Vec<Spec> {
        let automaton = c.automaton();
        let (mut specs, mut after) = (Vec::new(), None::<Positions>);
        for (at, child) in r.path(c.kind) {
            let here = match after {
                None => at.filter(|p| automaton.last.contains(p)),
                Some(after) => at.filter(|p| automaton.follow[p].intersects(after)),
            };
            let here = if here.is_empty() { at } else { here };
            if child {
                specs.push(self.taking_at(r, c.kind, here, outer).expect("a child step stands at children"));
            }
            after = Some(here);
        }
        specs.reverse();
        specs
    }

    /// Gives `piece` to the top reading, standing at the positions reading it: those that may hold
    /// it stay. What none may hold stands in an `Error`, which any may.
    fn deliver(&mut self, piece: u32) {
        let Some(Frame::Reading(mut r)) = self.stack.pop() else {
            unreachable!("an operand stands on a reading")
        };
        let outer = self.outer(self.stack.len()).copied();
        let content = Content::Node(piece);
        let holding: Vec<Positions> =
            r.candidates.iter().map(|&c| self.holding(&r, c, content, outer.as_ref())).collect();
        let piece = if holding.iter().all(|at| at.is_empty()) {
            self.misplaced(piece)
        } else {
            r.move_to(holding);
            piece
        };
        r.pieces.push(Piece::Child(piece));
        r.moved(true);
        self.stack.push(Frame::Reading(r));
    }
}
