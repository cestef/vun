//! Who reads the current token, by one rule: the outermost reading that reads it where it stands,
//! every frame above it able to end; else the innermost operand a form may continue; else the top
//! frame ends where it may, and is mended where it may not.

use vun_grammar::{KindSet, Positions, Read};

use super::frame::{Candidate, Content, Frame, Reading};
use super::spec::Spec;
use super::tree::Piece;
use crate::parser::Parser;

/// What the frames do at the current token.
#[derive(Debug)]
pub(super) enum Decision {
    /// The reading at this depth reads it, the frames above it ending.
    Read(usize),
    /// The operand at this depth goes on as a form of these kinds, the frames above it ending.
    Continue(usize, KindSet),
    /// The top frame ends.
    End,
    /// The top frame may neither go on nor end.
    Mend,
}

impl Parser<'_, '_, '_> {
    pub(super) fn decide(&self) -> Decision {
        let top = self.stack.len() - 1;
        let (mut reader, mut continuation, mut top_ends) = (None, None, false);
        let mut content = Content::Kinds { kinds: KindSet::EMPTY, ends: KindSet::EMPTY };
        for (i, frame) in self.stack.iter().enumerate().rev() {
            let ends = match frame {
                Frame::Reading(r) => {
                    let outer = self.outer(i);
                    let at =
                        |c: Candidate| if i == top { c.at } else { Some(self.holding(r, c, content, outer)) };
                    let reads = r.candidates.iter().any(|&c| self.reads_next(r, c.kind, at(c), outer));
                    if reads {
                        reader = Some(i);
                    }
                    let kinds = r.ending(at);
                    content = Content::Kinds {
                        kinds,
                        ends: self.ending_with(r, kinds, (i != top).then_some(content)),
                    };
                    !kinds.is_empty()
                }
                Frame::Operand(o) => {
                    content = o.piece.map_or(content, Content::Node);
                    if continuation.is_none() {
                        continuation = self.continuation(o, content).map(|kinds| (i, kinds));
                    }
                    self.held_below(i, content)
                }
            };
            top_ends |= i == top && ends;
            if !ends {
                break;
            }
        }
        match (reader, continuation) {
            (Some(i), _) => Decision::Read(i),
            (None, Some((i, kinds))) => Decision::Continue(i, kinds),
            (None, None) if top_ends => Decision::End,
            (None, None) => Decision::Mend,
        }
    }

    /// The kinds of the nodes a reading ending as `kinds` would end with: for a transparent one,
    /// those its last child ends with, which the operand above it holds, `above`, or it read.
    fn ending_with(&self, r: &Reading, kinds: KindSet, above: Option<Content>) -> KindSet {
        let transparent =
            kinds.iter().filter(|&k| self.transparent(k, r.anchor)).fold(KindSet::EMPTY, KindSet::with);
        if transparent.is_empty() {
            return kinds;
        }
        let last =
            || r.pieces.iter().rev().find_map(|p| if let Piece::Child(c) = p { Some(*c) } else { None });
        let child = match above {
            Some(Content::Kinds { ends, .. }) => ends,
            Some(Content::Node(n)) => self.tree.ends(n, self.tokens),
            None => last().map_or(KindSet::EMPTY, |n| self.tree.ends(n, self.tokens)),
        };
        kinds.without(transparent).union(child)
    }

    /// What the operand below the frame at depth `i` takes, if one is.
    pub(super) fn outer(&self, i: usize) -> Option<&Spec> {
        match self.stack.get(i.checked_sub(1)?)? {
            Frame::Operand(o) => Some(&o.spec),
            Frame::Reading(_) => None,
        }
    }

    /// The positions of `c`, reading a child, that may hold `content`.
    pub(super) fn holding(
        &self,
        r: &Reading,
        c: Candidate,
        content: Content,
        outer: Option<&Spec>,
    ) -> Positions {
        let holds = |p: usize| self.taking(r, c.kind, p, outer).is_some_and(|spec| self.held(content, &spec));
        c.at.unwrap_or_default().filter(holds)
    }

    /// Whether the reading below the operand at depth `i` may hold `content`.
    fn held_below(&self, i: usize, content: Content) -> bool {
        let Some(Frame::Reading(r)) = i.checked_sub(1).and_then(|below| self.stack.get(below)) else {
            return false;
        };
        let outer = self.outer(i - 1);
        r.candidates.iter().any(|&c| !self.holding(r, c, content, outer).is_empty())
    }

    /// Whether a reading of `kind` standing at `at` reads the current token next, as a token or
    /// as a child starting with it.
    pub(super) fn reads_next(
        &self,
        r: &Reading,
        kind: vun_grammar::NodeKind,
        at: Option<Positions>,
        outer: Option<&Spec>,
    ) -> bool {
        let Some(automaton) = kind.automaton() else { return false };
        automaton.next(at).iter().any(|p| match &automaton.reads[p] {
            Read::Child(_) => self.taking(r, kind, p, outer).is_some_and(|spec| self.starts(&spec, 0)),
            read => self.reads_token(read, 0),
        })
    }
}
