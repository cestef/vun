//! Where a node starts: what a position takes, the kinds that may start there at a token, and
//! the forms continuing an operand read before them.

use vun_grammar::{KindSet, Level, NodeKind, Positions, Read, TokenKind};

use super::frame::{Candidate, Content, Operand, Reading};
use super::spec::Spec;
use crate::cst::Anchor;
use crate::parser::Parser;

impl Parser<'_, '_, '_> {
    /// Whether `read` reads the token `ahead` of the current one: a token of its kinds, or a name
    /// or keyword spelled as one of its words.
    pub(super) fn reads_token(&self, read: &Read, ahead: usize) -> bool {
        match read {
            Read::Token(set) => set.contains(self.nth(ahead)),
            Read::Word(word) => word.spelled(self.nth(ahead), self.text(self.current_plus(ahead))),
            Read::Child(_) => false,
        }
    }

    /// The kinds a node read where `spec` says may be, starting at the token `ahead`: those
    /// reachable, and the transparent kinds standing for one named where written so.
    pub(super) fn alone(&self, spec: &Spec, ahead: usize) -> KindSet {
        let token = self.nth(ahead);
        if spec.starting.is_some_and(|s| !s.contains(token)) {
            return KindSet::EMPTY;
        }
        let set = spec.reachable().union(spec.standing_for().intersection(token.transparent()));
        let by_token = set.intersection(token.opening());
        let worded = set.intersection(token.worded()).without(by_token);
        if worded.is_empty() {
            return by_token;
        }
        let text = self.text(self.current_plus(ahead));
        worded.iter().filter(|k| k.opens_with(token, text)).fold(by_token, KindSet::with)
    }

    /// Whether a child where `spec` says may start at the token `ahead`; a token the lexer
    /// reported stands for any operand.
    pub(super) fn starts(&self, spec: &Spec, ahead: usize) -> bool {
        (self.nth(ahead) == TokenKind::Error && spec.takes_operands()) || !self.alone(spec, ahead).is_empty()
    }

    /// What position `p` of the candidate `c` of a reading takes: its child at its level, and for the last
    /// child of a transparent node, what the operand holding the node takes, `outer`, too.
    pub(super) fn taking(
        &self,
        reading: &Reading,
        c: Candidate,
        p: usize,
        outer: Option<&Spec>,
    ) -> Option<Spec> {
        let (kind, automaton) = (c.kind, c.automaton);
        let Read::Child(child) = &automaton.reads[p] else { return None };
        let edge = automaton.last.contains(p);
        let level = if edge { self.edge_level(kind, reading.anchor) } else { Level::LOWEST };
        match outer {
            Some(outer) if edge && self.transparent(kind, reading.anchor) => {
                let outer = outer.at(level);
                // Where the node may stand only as what it ends with, the child takes that alone.
                let own = Spec::child(child, level);
                Some(if outer.standing_for().contains(kind) { outer } else { own.union(outer) })
            }
            _ => Some(Spec::child(child, level)),
        }
    }

    /// What the positions `at` of the candidate `c` of a reading take, merged.
    pub(super) fn taking_at(
        &self,
        reading: &Reading,
        c: Candidate,
        at: Positions,
        outer: Option<&Spec>,
    ) -> Option<Spec> {
        at.iter().filter_map(|p| self.taking(reading, c, p, outer)).reduce(Spec::union)
    }

    /// How tightly a node of `kind` anchored at `anchor` holds the child at its end: as its level
    /// says, and for the infix form, tighter than its operator, whose level groups left.
    fn edge_level(&self, kind: NodeKind, anchor: Option<Anchor>) -> Level {
        let operator = anchor.map(|a| self.kind(a.token()).infix());
        match operator {
            Some(Some(op)) if kind == NodeKind::INFIX_FORM => op.level.tighter(),
            _ => kind.operand_level(),
        }
    }

    /// Whether a node of `kind` anchored at `anchor` is transparent.
    pub(super) fn transparent(&self, kind: NodeKind, anchor: Option<Anchor>) -> bool {
        let Some(Anchor::Token(t)) = anchor else { return false };
        self.kind(t).transparent().contains(kind)
    }

    /// The level of a form of `kind` continuing at the current token: the operator's for the
    /// infix form.
    fn level(&self, kind: NodeKind) -> Option<Level> {
        if kind == NodeKind::INFIX_FORM { self.peek().infix().map(|op| op.level) } else { kind.level() }
    }

    /// The forms continuing `o`, holding `content`, at the current token: of those it may reach at
    /// its level or tighter, whose leading positions may hold what it holds, those reading the
    /// longest run of tokens.
    pub(super) fn continuation(&self, o: &Operand, content: Content) -> Option<KindSet> {
        if self.at(TokenKind::Eof) {
            return None;
        }
        let reachable = o.spec.reachable();
        let mut best = (0, KindSet::EMPTY);
        for &kind in NodeKind::continuing_by_rank() {
            if !reachable.contains(kind) || self.level(kind).is_some_and(|l| l < o.spec.level) {
                continue;
            }
            let Some(automaton) = kind.automaton() else { continue };
            let held = self.leading(kind, content);
            let Some(run) = (!held.is_empty()).then(|| self.run(automaton, held)).flatten() else { continue };
            if run > best.0 {
                best = (run, KindSet::EMPTY);
            }
            if run == best.0 {
                best.1 = best.1.with(kind);
            }
        }
        (!best.1.is_empty()).then_some(best.1)
    }

    /// The leading positions of the production of `kind` that may hold `content`.
    pub(super) fn leading(&self, kind: NodeKind, content: Content) -> Positions {
        let Some(automaton) = kind.automaton() else { return Positions::EMPTY };
        let holds = |p: usize| match &automaton.reads[p] {
            Read::Child(child) => self.held(content, &Spec::child(child, Level::LOWEST)),
            _ => false,
        };
        automaton.leading.filter(holds)
    }

    /// Whether `content` may stand where `spec` says: a node as one of the kinds it spells, the
    /// kinds of a reading as one of them.
    pub(super) fn held(&self, content: Content, spec: &Spec) -> bool {
        match content {
            Content::Node(n) => self.tree.fits(n, spec, self.tokens),
            Content::Kinds { kinds, ends } => spec.may_hold(kinds, ends),
        }
    }
}
