//! The tokens a node of each kind may start with, and those an operand may.

use crate::{NodeKind, TRANSPARENT, TokenKind};

use super::derived::DERIVED;
use super::{Automaton, Child, KindSet, Positions, Read, TokenSet, Word};

/// What starts what: each kind's first tokens, by index, and those of any operand.
pub(super) struct Firsts {
    kinds: Vec<TokenSet>,
    operand: TokenSet,
}

impl Firsts {
    /// Computed together, to a fixed point: a production starting with a child starts as the
    /// child.
    pub(super) fn new(automata: &[Option<Automaton>], operands: KindSet) -> Self {
        let mut firsts = Self { kinds: vec![TokenSet::EMPTY; NodeKind::ALL.len()], operand: TokenSet::EMPTY };
        loop {
            let before = (firsts.kinds.clone(), firsts.operand);
            for (i, automaton) in automata.iter().enumerate() {
                if let Some(automaton) = automaton {
                    firsts.kinds[i] = firsts.of(automaton, automaton.first);
                }
            }
            firsts.operand =
                operands.iter().fold(TokenSet::EMPTY, |set, k| set.union(firsts.kinds[k.index()]));
            if (firsts.kinds.clone(), firsts.operand) == before {
                return firsts;
            }
        }
    }

    /// The tokens the positions `at` of `automaton` may start with.
    pub(super) fn of(&self, automaton: &Automaton, at: Positions) -> TokenSet {
        at.iter().fold(TokenSet::EMPTY, |set, p| set.union(self.read(&automaton.reads[p])))
    }

    fn read(&self, read: &Read) -> TokenSet {
        match read {
            Read::Token(tokens) | Read::Word(Word { tokens, .. }) => *tokens,
            Read::Child(child) => self.child(child),
        }
    }

    fn child(&self, child: &Child) -> TokenSet {
        if let Some(starting) = child.starting {
            return starting;
        }
        let kinds =
            |set: KindSet| set.iter().fold(TokenSet::EMPTY, |all, k| all.union(self.kinds[k.index()]));
        match child.kinds {
            // A transparent node stands for what it ends with, and starts with its own tokens.
            Some(set) if !child.exact => {
                TRANSPARENT.iter().fold(kinds(set), |all, row| all.union(TokenSet::of(row.tokens)))
            }
            Some(set) => kinds(set),
            None => self.operand.union(kinds(child.admitted)),
        }
    }
}

/// What a node of a kind may start with where no operand was read before it: its first tokens
/// and words, and those of first children that are no operands (`mut` in `mut x := 1`).
#[derive(Clone, Default, PartialEq, Eq)]
pub(super) struct Opening {
    pub(super) tokens: TokenSet,
    pub(super) words: Vec<Word>,
}

impl Opening {
    /// Every kind's, by index, to a fixed point: a production starting with a child starts as
    /// the child.
    pub(super) fn all(automata: &[Option<Automaton>]) -> Vec<Self> {
        let mut openings = vec![Self::default(); automata.len()];
        loop {
            let before = openings.clone();
            for (i, automaton) in automata.iter().enumerate() {
                let Some(a) = automaton else { continue };
                let mut opening = Self::default();
                for p in a.first.without(a.leading).iter() {
                    match &a.reads[p] {
                        Read::Token(tokens) => opening.tokens = opening.tokens.union(*tokens),
                        Read::Word(word) => opening.add(*word),
                        Read::Child(child) => {
                            for k in child.kinds.unwrap_or_default().iter() {
                                opening.merge(&openings[k.index()]);
                            }
                        }
                    }
                }
                openings[i] = opening;
            }
            if openings == before {
                return openings;
            }
        }
    }

    fn add(&mut self, word: Word) {
        if !self.words.contains(&word) {
            self.words.push(word);
        }
    }

    fn merge(&mut self, other: &Self) {
        self.tokens = self.tokens.union(other.tokens);
        for &word in &other.words {
            self.add(word);
        }
    }
}

impl TokenKind {
    /// The kinds that may start alone with this token, whatever its spelling.
    #[must_use]
    pub fn opening(self) -> KindSet {
        DERIVED.opening[self as usize]
    }

    /// The kinds that may start alone with this token when it is spelled as one of their words.
    #[must_use]
    pub fn worded(self) -> KindSet {
        DERIVED.worded[self as usize]
    }
}

impl NodeKind {
    /// Whether a node of this kind may start alone with a token of kind `token` spelled `text`,
    /// as one of its words.
    #[must_use]
    pub fn opens_with(self, token: TokenKind, text: &str) -> bool {
        DERIVED.openings[self.index()].words.iter().any(|word| word.spelled(token, text))
    }

    /// The kinds that stand as operands: with a production, no part, no root, no `error`.
    #[must_use]
    pub fn operands() -> KindSet {
        DERIVED.operands
    }

    /// Whether this kind is a tree's root: its production reads the end of the file.
    #[must_use]
    pub fn is_root(self) -> bool {
        self.automaton().is_some_and(Automaton::reads_end)
    }

    /// The tokens a node of this kind may start with.
    #[must_use]
    pub fn first(self) -> TokenSet {
        DERIVED.firsts.kinds[self.index()]
    }
}

impl Child {
    /// The tokens this child may start with.
    #[must_use]
    pub fn first(&self) -> TokenSet {
        DERIVED.firsts.child(self)
    }
}

impl Automaton {
    /// The tokens the positions `at` may start with.
    #[must_use]
    pub fn first_of(&self, at: Positions) -> TokenSet {
        DERIVED.firsts.of(self, at)
    }

    /// Whether it reads the end of the file.
    pub(super) fn reads_end(&self) -> bool {
        self.reads.iter().any(|r| matches!(r, Read::Token(t) if t.contains(TokenKind::Eof)))
    }
}
