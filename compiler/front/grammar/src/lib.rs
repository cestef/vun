//! Token kinds, node kinds and parse tables, generated from `lib/lang/lang/syntax/grammar.vun`.
//!
//! The grammar is defined once, as Vun data (spec 13, section 5). `cargo gen` writes
//! the kinds and tables into `generated/`, a module per declaration; this file derives lookups
//! from them.

use std::collections::HashMap;
use std::sync::LazyLock;

#[rustfmt::skip]
mod generated;

pub use generated::*;

/// Separates the members of a path, as written: `std.prelude`.
pub const PATH_SEPARATOR: &str = TokenKind::Dot.text().unwrap();

/// Punctuation by its first byte, longest spelling first, so the first prefix match is the
/// longest one.
static PUNCTUATION_BY_FIRST: LazyLock<HashMap<u8, Vec<TokenKind>>> = LazyLock::new(|| {
    let mut kinds = TokenKind::PUNCTUATION.to_vec();
    kinds.sort_by_key(|k| std::cmp::Reverse(k.text().map_or(0, str::len)));
    let mut by_first: HashMap<u8, Vec<TokenKind>> = HashMap::new();
    for kind in kinds {
        if let Some(&first) = kind.text().and_then(|t| t.as_bytes().first()) {
            by_first.entry(first).or_default().push(kind);
        }
    }
    by_first
});

/// Keywords by their spelling.
static KEYWORDS_BY_TEXT: LazyLock<HashMap<&'static str, TokenKind>> =
    LazyLock::new(|| TokenKind::KEYWORDS.iter().filter_map(|&k| Some((k.text()?, k))).collect());

/// Binding strength of an infix operator; higher binds tighter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Prec(u8);

impl Prec {
    pub const LOWEST: Self = Self(0);

    /// The next tighter level, for the right operand of a left-associative operator.
    #[must_use]
    pub const fn tighter(self) -> Self {
        Self(self.0 + 1)
    }
}

/// How an infix operator binds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Infix {
    pub prec: Prec,
    pub assoc: Assoc,
}

impl Context {
    /// Whether `word` is a keyword in this context.
    #[must_use]
    pub fn keyword(self, word: &str) -> bool {
        CONTEXTUAL.iter().any(|row| row.text == word && row.context == self)
    }
}

impl NodeKind {
    #[must_use]
    pub fn is_leaf(self) -> bool {
        Self::LEAVES.contains(&self)
    }
}

impl TokenKind {
    /// The punctuation spelled by the longest prefix of `rest`.
    #[must_use]
    pub fn punctuation(rest: &str) -> Option<Self> {
        let candidates = PUNCTUATION_BY_FIRST.get(rest.as_bytes().first()?)?;
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

    #[must_use]
    pub fn infix(self) -> Option<Infix> {
        INFIX.iter().zip(0..).find_map(|(row, level)| {
            row.tokens.contains(&self).then_some(Infix { prec: Prec(level), assoc: row.assoc })
        })
    }

    /// The token closing a group this token opens.
    #[must_use]
    pub fn closer(self) -> Option<Self> {
        DELIMITERS.iter().find_map(|row| (row.open == self).then_some(row.close))
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
