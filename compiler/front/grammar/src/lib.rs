//! Token kinds, node kinds and operator tables, generated from `lib/lang/lang/syntax/grammar.ov`.
//!
//! The grammar is defined once, as Overt data (spec 13, section 5). `cargo xtask grammar` writes
//! the kinds and tables into `generated.rs`; this file derives lookups from them.

use std::sync::LazyLock;

#[rustfmt::skip]
mod generated;

pub use generated::*;

/// Punctuation, longest spelling first, so the first prefix match is the longest one.
static PUNCTUATION_BY_LENGTH: LazyLock<Vec<TokenKind>> = LazyLock::new(|| {
    let mut kinds = TokenKind::PUNCTUATION.to_vec();
    kinds.sort_by_key(|k| std::cmp::Reverse(k.text().map_or(0, str::len)));
    kinds
});

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

/// Whether `word` is a keyword in `context`.
#[must_use]
pub fn contextual(word: &str, context: Context) -> bool {
    CONTEXTUAL.contains(&(word, context))
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
        PUNCTUATION_BY_LENGTH.iter().copied().find(|k| k.text().is_some_and(|t| rest.starts_with(t)))
    }

    /// The keyword spelled `word`, if `word` is reserved.
    #[must_use]
    pub fn keyword(word: &str) -> Option<Self> {
        Self::KEYWORDS.iter().copied().find(|k| k.text() == Some(word))
    }

    #[must_use]
    pub fn is_trivia(self) -> bool {
        Self::TRIVIA.contains(&self)
    }

    #[must_use]
    pub fn infix(self) -> Option<Infix> {
        INFIX.iter().zip(0..).find_map(|(&(assoc, kinds), level)| {
            kinds.contains(&self).then_some(Infix { prec: Prec(level), assoc })
        })
    }

    /// The token closing a group this token opens.
    #[must_use]
    pub fn closer(self) -> Option<Self> {
        DELIMITERS.iter().find_map(|&(open, close)| (open == self).then_some(close))
    }

    #[must_use]
    pub fn opens(self) -> bool {
        self.closer().is_some()
    }

    #[must_use]
    pub fn closes(self) -> bool {
        DELIMITERS.iter().any(|&(_, close)| close == self)
    }
}

#[cfg(test)]
mod tests;
