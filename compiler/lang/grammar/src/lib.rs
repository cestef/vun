//! Token kinds, node kinds and parse tables, generated from `lib/lang/lang/syntax/grammar.vun`.
//!
//! The grammar is defined once, as Vun data (spec 13, section 5). `cargo gen` writes
//! the kinds and tables into `generated/`, a module per declaration; this file derives lookups
//! from them.

use std::sync::LazyLock;

#[rustfmt::skip]
mod generated;
mod automaton;
mod choice;
mod level;
mod production;

pub use automaton::{Automaton, Child, KindSet, Positions, Read, TokenSet, Word};
pub use generated::*;
pub use level::{Infix, Level};
pub use production::Element;

/// Separates the members of a path, as written: `std.prelude`.
pub const PATH_SEPARATOR: &str = TokenKind::Dot.text().unwrap();

/// Punctuation by its first byte, longest spelling first, so the first prefix match is the
/// longest one.
static PUNCTUATION_BY_FIRST: LazyLock<Vec<Vec<TokenKind>>> = LazyLock::new(|| {
    let mut kinds = TokenKind::PUNCTUATION.to_vec();
    kinds.sort_by_key(|k| std::cmp::Reverse(k.text().map_or(0, str::len)));
    let mut by_first = vec![Vec::new(); usize::from(u8::MAX) + 1];
    for kind in kinds {
        if let Some(&first) = kind.text().and_then(|t| t.as_bytes().first()) {
            by_first[usize::from(first)].push(kind);
        }
    }
    by_first
});

/// Keywords by their spelling.
static KEYWORDS_BY_TEXT: LazyLock<foldhash::HashMap<&'static str, TokenKind>> =
    LazyLock::new(|| TokenKind::KEYWORDS.iter().filter_map(|&k| Some((k.text()?, k))).collect());

impl Context {
    /// Whether `word` is a keyword in this context.
    #[must_use]
    pub fn keyword(self, word: &str) -> bool {
        CONTEXTUAL.iter().any(|row| row.text == word && row.context == self)
    }
}

impl BorrowWord {
    /// The word spelled `text`, if one is.
    #[must_use]
    pub fn spelled(text: &str) -> Option<Self> {
        Self::QUALIFIERS.iter().chain(Self::MODES).copied().find(|w| w.text() == Some(text))
    }
}

impl NodeKind {
    #[must_use]
    pub fn is_leaf(self) -> bool {
        Self::LEAVES.contains(&self)
    }

    /// Whether a node of this kind is a bare token, standing in no field: `punct`, one its
    /// parent's production names, or `skipped`, one no production reads.
    #[must_use]
    pub fn is_token(self) -> bool {
        matches!(self, Self::Punct | Self::Skipped)
    }
}

impl TokenKind {
    /// The punctuation spelled by the longest prefix of `rest`.
    #[must_use]
    pub fn punctuation(rest: &str) -> Option<Self> {
        let candidates = &PUNCTUATION_BY_FIRST[usize::from(*rest.as_bytes().first()?)];
        candidates.iter().copied().find(|k| k.text().is_some_and(|t| rest.starts_with(t)))
    }

    /// The keyword spelled `word`, if `word` is reserved.
    #[must_use]
    pub fn keyword(word: &str) -> Option<Self> {
        KEYWORDS_BY_TEXT.get(word).copied()
    }

    #[must_use]
    pub fn is_trivia(self) -> bool {
        Self::TRIVIA.contains(&self)
    }

    /// The token closing a group this token opens.
    #[must_use]
    pub fn closer(self) -> Option<Self> {
        self.delimiter().map(|row| row.close)
    }

    /// The row of `delimiters` of the group this token opens.
    #[must_use]
    pub fn delimiter(self) -> Option<DelimitersRow> {
        DELIMITERS.iter().copied().find(|row| row.open == self)
    }

    #[must_use]
    pub fn is_keyword(self) -> bool {
        Self::KEYWORDS.contains(&self)
    }

    #[must_use]
    pub fn opens(self) -> bool {
        self.closer().is_some()
    }

    #[must_use]
    pub fn closes(self) -> bool {
        DELIMITERS.iter().any(|row| row.close == self)
    }
}

#[cfg(test)]
mod tests;
