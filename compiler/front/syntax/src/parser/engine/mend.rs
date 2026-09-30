//! Recovery, where the top frame may neither go on nor end, each rule reporting once per
//! position. A reading supposes absent what a next position reads, where the token may be read
//! after it; before a token a frame below reads, or the end of the file, it completes along its
//! shortest way out up to the token, as before the token closing a group it opened, and within
//! that group only before punctuation a frame below reads; else it skips the token, with its
//! group. In the fallout of an error, until the
//! productions read a punctuation token in place, nothing is supposed: readings complete or skip.
//! An operand holding what may not stand there goes on as a kind named there where that reads
//! the token after supposing what it reads first, and is held in an `Error` otherwise.

use vun_diag::{ChildKind, Diagnostic, Msg, errors};
use vun_grammar::{KindSet, NodeKind, Positions, Read, TokenKind, TokenSet};

use super::expected::Expected;
use super::frame::{Candidate, Content, Frame, Operand, Reading};
use super::spec::Spec;
use super::tree::Piece;
use crate::cst::Anchor;
use crate::parser::{MAX_NESTING, Parser};
use crate::tokens::TokenId;

/// Where skipping the rest of an expression too deep stops, besides a closing delimiter.
const EXPRESSION_ENDS: &[TokenKind] = &[TokenKind::Semi, TokenKind::Comma];

impl Parser<'_, '_, '_> {
    pub(super) fn mend(&mut self) {
        match self.stack.pop().expect("a frame mends") {
            Frame::Reading(mut r) => {
                // A token the lexer reported starts the fallout of that report.
                self.quiet |= self.tokens.diagnosed(self.current());
                let outer = self.outer(self.stack.len()).copied();
                let supposing = !self.quiet && r.supposed != Some(self.pos);
                if !(supposing && self.suppose(&mut r, outer.as_ref())) {
                    let ends = self.at(TokenKind::Eof)
                        || self.group(&r).is_some_and(|(_, close)| close == self.peek())
                        || self.read_below(&r, self.group(&r).is_some());
                    if !(ends && self.complete_way(&mut r)) {
                        self.report_expected_after(&r);
                        self.skip_group(&mut r);
                    }
                }
                self.stack.push(Frame::Reading(r));
            }
            Frame::Operand(o) => self.mend_operand(o),
        }
    }

    /// Supposes absent what a next position of `r` reads, where the current token may be read
    /// after it: a token, or a child as an `Error`.
    fn suppose(&mut self, r: &mut Reading, outer: Option<&Spec>) -> bool {
        let Some((child, supposing)) = self.supposing(r, outer) else { return false };
        r.supposed = Some(self.pos);
        self.report_expected_after(r);
        r.move_to(supposing);
        self.absent(r, child);
        true
    }

    /// Where each candidate of `r` would stand, supposing absent what one of its next positions
    /// reads that the current token may be read after, those of the first reading a child or
    /// not alike; and whether they read a child. `None` where none may be supposed.
    fn supposing(&self, r: &Reading, outer: Option<&Spec>) -> Option<(bool, Vec<Positions>)> {
        let after = |c: Candidate, q: usize| self.reads_next(r, c, Some(Positions::single(q)), outer);
        let supposing: Vec<Positions> =
            r.candidates.iter().map(|&c| c.next().filter(|q| after(c, q))).collect();
        let child = |c: Candidate, q: usize| matches!(c.automaton.reads[q], Read::Child(_));
        let first =
            r.candidates.iter().zip(&supposing).find_map(|(&c, at)| Some(child(c, at.iter().next()?)))?;
        let shaped = r.candidates.iter().zip(supposing).map(|(&c, at)| at.filter(|q| child(c, q) == first));
        Some((first, shaped.collect()))
    }

    /// Completes `r` along the shortest way to its end, up to a position reading the current
    /// token, reporting the first thing absent on it.
    fn complete_way(&mut self, r: &mut Reading) -> bool {
        let ways = r.candidates.iter().filter_map(|&c| Some((c, c.automaton.way_out(c.at)?)));
        let Some((c, way)) = ways.min_by_key(|(_, way)| way.len()) else { return false };
        let reads = &c.automaton.reads;
        let closer = self.group(r);
        match (way.first().map(|&q| &reads[q]), closer) {
            (Some(Read::Token(set)), Some((open, close))) if set.contains(close) => {
                self.report_unclosed(open, close);
            }
            _ => self.report_expected_after(r),
        }
        r.candidates = vec![c];
        let outer = self.outer(self.stack.len()).copied();
        for q in way {
            if self.reads_next(r, c, r.candidates[0].at, outer.as_ref()) {
                break;
            }
            r.move_to([Positions::single(q)]);
            self.absent(r, matches!(reads[q], Read::Child(_)));
        }
        true
    }

    /// Adds to `r` what it now stands at, absent before the current token: a child as an `Error`,
    /// or a token, its anchor where it anchors it.
    fn absent(&mut self, r: &mut Reading, child: bool) {
        let token = self.current();
        let anchors = r.candidates.iter().any(|c| c.at.unwrap_or_default().intersects(c.automaton.anchors));
        if child {
            r.pieces.push(Piece::Child(self.tree.leaf(NodeKind::Error, Anchor::Missing(token))));
        } else if anchors && r.anchor.is_none() {
            r.anchor = Some(Anchor::Missing(token));
        } else {
            r.pieces.push(Piece::Absent(token));
        }
        r.moved(child);
    }

    /// The group `r` opened last and has not closed: the token it took opening it, and its
    /// closer.
    fn group(&self, r: &Reading) -> Option<(TokenId, TokenKind)> {
        let taken = r.pieces.iter().filter_map(|p| if let Piece::Token(t) = p { Some(*t) } else { None });
        let mut open: Vec<TokenId> = Vec::new();
        for t in r.anchor.map(Anchor::token).into_iter().chain(taken) {
            if self.kind(t).opens() {
                open.push(t);
            } else if self.kind(t).closes() {
                open.pop();
            }
        }
        let opened = open.pop()?;
        Some((opened, self.kind(opened).closer()?))
    }

    /// Whether a reading below `r`, standing where what the reading above it may complete as
    /// would put it, reads the current token: as punctuation only, where `r` opened a group
    /// what starts a child belongs to.
    fn read_below(&self, r: &Reading, grouped: bool) -> bool {
        let kinds = |r: &Reading| r.candidates.iter().fold(KindSet::EMPTY, |set, c| set.with(c.kind));
        let mut content = Content::Kinds { kinds: kinds(r), ends: kinds(r) };
        for (i, frame) in self.stack.iter().enumerate().rev() {
            if let Frame::Reading(below) = frame {
                let outer = self.outer(i);
                let at = |c: Candidate| self.holding(below, c, content, outer);
                let reads = |c: Candidate| {
                    let next = c.automaton.next(Some(at(c)));
                    if grouped {
                        next.iter().any(|p| self.reads_token(&c.automaton.reads[p], 0))
                    } else {
                        self.reads_next(below, c, Some(at(c)), outer)
                    }
                };
                if below.candidates.iter().any(|&c| reads(c)) {
                    return true;
                }
                content = Content::Kinds { kinds: kinds(below), ends: kinds(below) };
            }
        }
        false
    }

    /// Skips the current token into `r`, with the rest of its group when it opens one.
    fn skip_group(&mut self, r: &mut Reading) {
        let mut depth = 0usize;
        loop {
            let token = self.peek();
            depth = if token.opens() {
                depth + 1
            } else if token.closes() {
                depth.saturating_sub(1)
            } else {
                depth
            };
            r.pieces.push(Piece::Skipped(self.bump()));
            if depth == 0 || self.at(TokenKind::Eof) {
                return;
            }
        }
    }

    /// Reports what the next positions of `r` expect.
    fn report_expected_after(&mut self, r: &Reading) {
        let expected = self.expected(r, self.outer(self.stack.len()));
        self.report_expected_what(expected);
    }

    /// The top operand holds what may not stand there, which is reported with the tokens the
    /// kinds named there would read after it (`:=` after `run()` among declarations). It goes on
    /// as the first of them that reads the current token after supposing one of those
    /// (`.some(v) v` among arms, supposing `=>`), else is held in an `Error`.
    fn mend_operand(&mut self, mut o: Operand) {
        let piece = o.piece.expect("an operand mends once read");
        let continuing = |kind: NodeKind| {
            let mut r = Reading::new(KindSet::EMPTY.with(kind));
            r.move_to([self.leading(kind, Content::Node(piece))]);
            r
        };
        let named: Vec<Reading> = NodeKind::by_rank()
            .iter()
            .filter(|&&k| o.spec.named.contains(k))
            .map(|&k| continuing(k))
            .collect();
        let named: Vec<&Reading> = named.iter().filter(|r| !r.candidates.is_empty()).collect();
        let tokens = named.iter().fold(TokenSet::EMPTY, |set, r| match self.expected(r, Some(&o.spec)) {
            Expected::Tokens(tokens) => set.union(tokens),
            _ => set,
        });
        if !tokens.is_empty() {
            self.report_expected_what(Expected::Tokens(tokens));
        }
        let going_on =
            named.iter().find(|r| self.supposing(r, Some(&o.spec)).is_some_and(|(child, _)| !child));
        let Some(kind) = going_on.map(|r| r.candidates[0].kind) else {
            o.piece = Some(self.misplaced(piece));
            self.stack.push(Frame::Operand(o));
            return;
        };
        let outer = o.spec;
        self.stack.push(Frame::Operand(o));
        self.continue_operand(KindSet::EMPTY.with(kind));
        if let Some(Frame::Reading(r)) = self.stack.last_mut() {
            let mut r = std::mem::replace(r, Reading::new(KindSet::EMPTY));
            self.suppose(&mut r, Some(&outer));
            *self.stack.last_mut().expect("the reading is on top") = Frame::Reading(r);
        }
    }

    /// Reports `piece`, which may not stand where it was read, and holds it in an `Error`.
    pub(super) fn misplaced(&mut self, piece: u32) -> u32 {
        let first = self.tree.first_token(piece);
        let msg = Msg::new().code(self.text(first).to_owned()).text(" cannot stand here");
        self.report(self.tokens.span(first), Diagnostic::error(errors::SYNTAX_EXPECTED_TOKEN, msg));
        self.tree.spell(NodeKind::Error, []);
        self.tree.add(Anchor::Missing(first), [Piece::Child(piece)])
    }

    /// Reports nesting past the limit at the current token, and makes `held` and the rest of the
    /// expression one `Error`.
    pub(super) fn nesting_error(&mut self, held: Option<u32>) -> u32 {
        let msg = Msg::new().text("nesting is deeper than ").num(MAX_NESTING).text(" levels");
        let note = Msg::new().text("each operand, each block and each link of a chain is one level");
        let span = self.tokens.span(self.current());
        self.report(
            span,
            Diagnostic::error(errors::SYNTAX_NESTING_TOO_DEEP, msg).child(ChildKind::Note, note),
        );
        let token = self.peek();
        if token == TokenKind::Eof || EXPRESSION_ENDS.contains(&token) || token.closes() {
            return held.unwrap_or_else(|| self.tree.leaf(NodeKind::Error, Anchor::Missing(self.current())));
        }
        let anchor = self.bump();
        let mut pieces: Vec<Piece> = held.map(Piece::Child).into_iter().collect();
        let mut depth = usize::from(self.kind(anchor).opens());
        while !self.at(TokenKind::Eof) {
            let token = self.peek();
            if depth == 0 && (EXPRESSION_ENDS.contains(&token) || token.closes()) {
                break;
            }
            depth = if token.opens() {
                depth + 1
            } else if token.closes() {
                depth - 1
            } else {
                depth
            };
            pieces.push(Piece::Skipped(self.bump()));
        }
        // What the frames around it miss where the skipping ends is part of this report.
        self.reported = Some(self.pos);
        self.tree.spell(NodeKind::Error, []);
        self.tree.add(Anchor::Token(anchor), pieces)
    }
}
