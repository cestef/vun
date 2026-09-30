//! The moves of the top frame: a reading takes the current token, or starts the child it reads
//! with it; an operand goes on as the forms continuing it.

use vun_grammar::{Assoc, KindSet, NodeKind, Positions, TokenKind};
use vun_trace::tracing;

use super::frame::{Candidate, Content, Frame, Operand, Reading};
use super::spec::Spec;
use super::tree::Piece;
use crate::cst::Anchor;
use crate::parser::{MAX_NESTING, Parser};

impl Parser<'_, '_, '_> {
    /// The top reading reads the current token: as a token where a position reads one and what
    /// may follow it is next, else as the child a position starts with it.
    pub(super) fn read_token(&mut self) {
        let Some(Frame::Reading(mut r)) = self.stack.pop() else { unreachable!("a reading reads") };
        let outer = self.outer(self.stack.len()).copied();
        let tokens = |c: Candidate| c.next().filter(|p| self.reads_token(&c.automaton.reads[p], 0));
        let children = |c: Candidate| {
            let starts = |p: usize| self.taking(&r, c, p, outer.as_ref()).is_some_and(|s| self.starts(&s, 0));
            c.next().filter(starts)
        };
        let reads_token = r.candidates.iter().any(|&c| !tokens(c).is_empty());
        let reads_child = r.candidates.iter().any(|&c| !children(c).is_empty());
        // Where both may, the token, where what follows it may be read after it.
        let after = |c: Candidate| {
            let at = tokens(c);
            let automaton = c.automaton;
            !at.is_empty()
                && (automaton.accepts(Some(at))
                    || automaton.first_of(automaton.next(Some(at))).contains(self.nth(1)))
        };
        let token = reads_token && (!reads_child || r.candidates.iter().any(|&c| after(c)));
        if token {
            let keep = |c: Candidate| if reads_child && !after(c) { Positions::EMPTY } else { tokens(c) };
            let moves: Vec<Positions> = r.candidates.iter().map(|&c| keep(c)).collect();
            let anchors = r.candidates.iter().zip(&moves).any(|(c, at)| at.intersects(c.automaton.anchors));
            r.move_to(moves);
            self.take(&mut r, anchors);
            r.moved(false);
            self.stack.push(Frame::Reading(r));
        } else if reads_child {
            let moves: Vec<Positions> = r.candidates.iter().map(|&c| children(c)).collect();
            r.move_to(moves);
            let specs = r
                .candidates
                .iter()
                .filter_map(|&c| self.taking_at(&r, c, c.at.unwrap_or_default(), outer.as_ref()));
            let spec = specs.reduce(Spec::union).expect("a child move has positions");
            self.stack.push(Frame::Reading(r));
            self.open(spec);
        } else {
            self.stack.push(Frame::Reading(r));
        }
    }

    /// Takes the current token into `r`: as its anchor where it anchors it and none did before,
    /// else as a token it holds, where the productions read on in place after an error. The end
    /// of the file anchors the root and is never taken.
    fn take(&mut self, r: &mut Reading, anchors: bool) {
        let token = self.current();
        if !self.at(TokenKind::Eof) {
            self.bump();
        }
        if anchors && r.anchor.is_none() {
            r.anchor = Some(Anchor::Token(token));
        } else {
            r.pieces.push(Piece::Token(token));
            self.quiet = false;
        }
    }

    /// Opens an operand taking `spec`, and the reading of the kinds that may start it; a token
    /// the lexer reported stands for it as an `Error`.
    fn open(&mut self, spec: Spec) {
        let mut operand = Operand { spec, piece: None, links: 0, chained: None };
        self.depth += 1;
        if self.depth >= MAX_NESTING {
            operand.piece = Some(self.nesting_error(None));
        } else if self.at(TokenKind::Error) {
            let token = self.bump();
            operand.piece = Some(self.tree.leaf(NodeKind::Error, Anchor::Token(token)));
        }
        let kinds = if operand.piece.is_some() { KindSet::EMPTY } else { self.alone(&spec, 0) };
        tracing::trace!(token = ?self.peek(), ?kinds, "opens");
        self.stack.push(Frame::Operand(operand));
        if !kinds.is_empty() {
            self.stack.push(Frame::Reading(Reading::new(kinds)));
        }
    }

    /// The top operand goes on as a form of one of `kinds`, holding what it holds at the leading
    /// positions that may; an operator of a level without grouping chaining another makes the
    /// form an `Error`.
    pub(super) fn continue_operand(&mut self, kinds: KindSet) {
        let Some(Frame::Operand(mut o)) = self.stack.pop() else { unreachable!("an operand goes on") };
        let piece = o.piece.expect("the operand read");
        let mut r = Reading::new(kinds);
        let leading: Vec<Positions> =
            r.candidates.iter().map(|c| self.leading(c.kind, Content::Node(piece))).collect();
        r.move_to(leading);
        if r.candidates.is_empty() {
            self.stack.push(Frame::Operand(o));
            return;
        }
        tracing::trace!(token = ?self.peek(), kinds = ?r.candidates, "continues");
        let op = kinds.contains(NodeKind::INFIX_FORM).then(|| self.peek().infix()).flatten();
        let op = op.filter(|op| op.assoc == Assoc::None).map(|op| op.level);
        if op.is_some() && o.chained == op {
            self.report_chained();
            r.wrong = true;
        }
        o.chained = op;
        o.links += 1;
        o.piece = None;
        if self.depth.saturating_add(o.links) >= MAX_NESTING {
            o.piece = Some(self.nesting_error(Some(piece)));
            self.stack.push(Frame::Operand(o));
            return;
        }
        r.pieces.push(Piece::Child(piece));
        r.moved(true);
        self.stack.push(Frame::Operand(o));
        self.stack.push(Frame::Reading(r));
    }
}
