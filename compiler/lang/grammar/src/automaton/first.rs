//! The tokens a node of each kind may start with, and those an operand may.

use std::sync::LazyLock;

use crate::{NodeKind, PARTS, TRANSPARENT, TokenKind};

use super::{Automaton, Child, KindSet, Read, TokenSet};

/// What starts what: each kind's first tokens, by index, and those of any operand.
struct Firsts {
    kinds: Vec<TokenSet>,
    operand: TokenSet,
}

/// The kinds that stand as operands: those with a production, no part, no root and no `error`.
static OPERANDS: LazyLock<KindSet> = LazyLock::new(|| {
    let parts = KindSet::of(PARTS);
    let operand = |k: &&NodeKind| {
        k.automaton().is_some() && !parts.contains(**k) && !k.is_root() && **k != NodeKind::Error
    };
    KindSet::of(&NodeKind::ALL.iter().filter(operand).copied().collect::<Vec<_>>())
});

/// Computed together, to a fixed point: a production starting with a child starts as the child.
static FIRSTS: LazyLock<Firsts> = LazyLock::new(|| {
    let mut firsts = Firsts { kinds: vec![TokenSet::EMPTY; NodeKind::ALL.len()], operand: TokenSet::EMPTY };
    loop {
        let before = (firsts.kinds.clone(), firsts.operand);
        for &kind in NodeKind::ALL {
            if let Some(automaton) = kind.automaton() {
                let first = automaton
                    .first
                    .iter()
                    .fold(TokenSet::EMPTY, |set, p| set.union(firsts.read(&automaton.reads[p])));
                firsts.kinds[kind.index()] = first;
            }
        }
        firsts.operand = OPERANDS.iter().fold(TokenSet::EMPTY, |set, k| set.union(firsts.kinds[k.index()]));
        if (firsts.kinds.clone(), firsts.operand) == before {
            return firsts;
        }
    }
});

impl Firsts {
    fn read(&self, read: &Read) -> TokenSet {
        match read {
            Read::Token(tokens) | Read::Word { tokens, .. } => *tokens,
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

impl NodeKind {
    /// The kinds that stand as operands: with a production, no part, no root, no `error`.
    #[must_use]
    pub fn operands() -> KindSet {
        *OPERANDS
    }

    /// Whether this kind is a tree's root: its production reads the end of the file.
    #[must_use]
    pub fn is_root(self) -> bool {
        self.automaton().is_some_and(|a| {
            a.reads.iter().any(|r| matches!(r, Read::Token(t) if t.contains(TokenKind::Eof)))
        })
    }

    /// The tokens a node of this kind may start with.
    #[must_use]
    pub fn first(self) -> TokenSet {
        FIRSTS.kinds[self.index()]
    }
}

impl Child {
    /// The tokens this child may start with.
    #[must_use]
    pub fn first(&self) -> TokenSet {
        FIRSTS.child(self)
    }
}

impl Automaton {
    /// The tokens the positions `at` may start with.
    #[must_use]
    pub fn first_of(&self, at: super::Positions) -> TokenSet {
        at.iter().fold(TokenSet::EMPTY, |set, p| set.union(FIRSTS.read(&self.reads[p])))
    }
}
