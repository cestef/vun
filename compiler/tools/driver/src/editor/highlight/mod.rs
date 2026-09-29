//! What each token of a file is, for an editor to color: its class and modifiers. Comments,
//! literals, keywords and operators are read from their tokens and the grammar's tables;
//! contextual words from the syntax tree; names from what they resolve to, a declaration's class
//! from its value as sema computes it.

mod lexical;
mod names;

use vun_span::{ByteRange, FileId};
use vun_variants::Variants;

use crate::Session;

/// A token's class and modifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Highlight {
    pub range: ByteRange,
    pub class: Class,
    pub modifiers: Modifiers,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Variants)]
pub enum Class {
    Namespace,
    Type,
    Function,
    Variable,
    Parameter,
    Property,
    Variant,
    Keyword,
    Comment,
    String,
    Number,
    Operator,
    Label,
    Attribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Variants)]
pub enum Modifier {
    /// Where the name is declared.
    Declaration,
    Mutable,
    /// Immutable, where the language allows either.
    Readonly,
    /// Known at compile time: `$`.
    Comptime,
    Documentation,
}

/// A set of modifiers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers(u8);

impl Modifiers {
    #[must_use]
    pub const fn with(self, m: Modifier) -> Self {
        Self(self.0 | 1 << m as u8)
    }

    /// This set, with `m` when `when`.
    #[must_use]
    pub const fn with_if(self, m: Modifier, when: bool) -> Self {
        if when { self.with(m) } else { self }
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn contains(self, m: Modifier) -> bool {
        self.0 & 1 << m as u8 != 0
    }

    pub fn iter(self) -> impl Iterator<Item = Modifier> {
        Modifier::ALL.iter().copied().filter(move |&m| self.contains(m))
    }
}

impl Session {
    /// The highlights of `file`, in source order, none overlapping.
    pub fn highlights(&mut self, file: FileId) -> Vec<Highlight> {
        let parsed = self.syntax(file);
        let names = names::Names::of(self, file, &parsed);
        let contextual = lexical::Contextual::of(&parsed);
        let mut highlights = Vec::new();
        for (lexeme, token) in lexical::lexemes(&parsed.tokens) {
            let classified = match token {
                Some(t) => contextual.token(&parsed.tokens, t).or_else(|| names.class(self, &parsed, t)),
                None => lexical::trivia(lexeme.kind),
            };
            if let Some((class, modifiers)) = classified {
                highlights.push(Highlight { range: lexeme.range, class, modifiers });
            }
        }
        highlights
    }
}
