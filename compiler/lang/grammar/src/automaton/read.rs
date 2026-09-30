//! What a position of a production reads: a token, a word or a child, with what the tables say
//! the child may be.

use crate::{ADMITS, HOLDS, NodeField, NodeKind, TokenKind};

use super::{KindSet, TokenSet};

/// What a position reads.
#[derive(Clone, Copy, Debug)]
pub enum Read {
    /// A token of one of these kinds.
    Token(TokenSet),
    Word(Word),
    Child(Child),
}

/// A name, or a keyword, spelled as one of `words`; `tokens` are the kinds they lex as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Word {
    pub words: &'static [&'static str],
    pub tokens: TokenSet,
}

impl Word {
    /// Whether a token of kind `token` spelled `text` is one of the words.
    #[must_use]
    pub fn spelled(&self, token: TokenKind, text: &str) -> bool {
        self.tokens.contains(token) && self.words.contains(&text)
    }
}

/// A child a position reads.
#[derive(Clone, Copy, Debug)]
pub struct Child {
    pub field: NodeField,
    /// The kinds it may be: the kind its field is named after, exactly, or those a table names
    /// (`.of`, `holds`), which the node it ends with through transparent kinds may be; any operand
    /// when `None`.
    pub kinds: Option<KindSet>,
    /// Whether `kinds` is the kind its field is named after, which no transparent node stands for.
    pub exact: bool,
    /// The tokens it must start with (`.starting`).
    pub starting: Option<TokenSet>,
    /// The parts it may be besides operands (`admits`).
    pub admitted: KindSet,
}

/// How a child term restricts its node.
pub(super) enum Restriction {
    None,
    Of(&'static [NodeKind]),
    Starting(&'static [TokenKind]),
}

impl Read {
    pub(super) fn word(words: &'static [&'static str]) -> Self {
        let lexed = |w: &&str| TokenKind::keyword(w).unwrap_or(TokenKind::Ident);
        Self::Word(Word { words, tokens: TokenSet::of(&words.iter().map(lexed).collect::<Vec<_>>()) })
    }

    /// The child in `field` of a node of `kind`, restricted so.
    pub(super) fn child(kind: NodeKind, field: NodeField, restriction: Restriction) -> Self {
        let named = kind.slots().iter().find(|s| s.field == field).and_then(|s| s.kind);
        let listed = |rows: &mut dyn Iterator<Item = &'static [NodeKind]>| {
            let kinds: Vec<NodeKind> = rows.flatten().copied().collect();
            (!kinds.is_empty()).then(|| KindSet::of(&kinds))
        };
        let held = listed(&mut HOLDS.iter().filter(|row| row.fields.contains(&field)).map(|row| row.kinds));
        let admitted =
            listed(&mut ADMITS.iter().filter(|row| row.fields.contains(&field)).map(|row| row.kinds));
        let child = |kinds, exact| Child {
            field,
            kinds,
            exact,
            starting: None,
            admitted: admitted.unwrap_or_default(),
        };
        Self::Child(match (named, restriction) {
            (Some(kind), _) => child(Some(KindSet::EMPTY.with(kind)), true),
            (None, Restriction::None) => child(held, false),
            (None, Restriction::Of(kinds)) => {
                let kinds = KindSet::of(kinds);
                child(Some(held.map_or(kinds, |held| held.intersection(kinds))), false)
            }
            (None, Restriction::Starting(tokens)) => {
                Child { starting: Some(TokenSet::of(tokens)), ..child(held, false) }
            }
        })
    }
}
