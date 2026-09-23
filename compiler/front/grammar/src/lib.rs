//! Token and node kinds, generated from `lib/lang/lang/syntax/grammar.ov`.
//!
//! The grammar is defined once, as Overt data (spec 13, section 5). Until `cargo xtask grammar`
//! generates this crate, the table below is maintained by hand and must match `grammar.ov`.

macro_rules! tokens {
    (
        trivia { $($trivia:ident),* $(,)? }
        atoms { $($atom:ident),* $(,)? }
        keywords { $($kw:ident = $kw_text:literal),* $(,)? }
        punctuation { $($punct:ident = $punct_text:literal),* $(,)? }
    ) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum TokenKind {
            $($trivia,)*
            $($atom,)*
            $($kw,)*
            $($punct,)*
            Eof,
        }

        impl TokenKind {
            pub const TRIVIA: &[Self] = &[$(Self::$trivia),*];
            pub const KEYWORDS: &[Self] = &[$(Self::$kw),*];
            pub const PUNCTUATION: &[Self] = &[$(Self::$punct),*];

            /// The fixed spelling of keywords and punctuation.
            #[must_use]
            pub const fn text(self) -> Option<&'static str> {
                match self {
                    $(Self::$kw => Some($kw_text),)*
                    $(Self::$punct => Some($punct_text),)*
                    _ => None,
                }
            }
        }
    };
}

tokens! {
    trivia { Whitespace, Comment }
    atoms { DocComment, ModuleDocComment, Ident, Int, Float, Char, String, RawStringLine, Label }
    keywords {
        Mut = "mut", If = "if", Else = "else", Match = "match", For = "for", Break = "break",
        Continue = "continue", Return = "return", Yield = "yield", Defer = "defer", And = "and",
        Or = "or", Struct = "struct", Enum = "enum", Unsafe = "unsafe",
    }
    punctuation {
        ColonEq = ":=", Colon = ":", Eq = "=", EqEq = "==", BangEq = "!=", Lt = "<", Le = "<=",
        Gt = ">", Ge = ">=", Arrow = "->", FatArrow = "=>", Dot = ".", DotStar = ".*", DotDot = "..",
        DotDotEq = "..=", DotLBrace = ".{", Comma = ",", Semi = ";", LParen = "(", RParen = ")",
        LBracket = "[", RBracket = "]", LBrace = "{", RBrace = "}", HashLBracket = "#[",
        HashLBrace = "#{", Dollar = "$", Question = "?", Amp = "&", Pipe = "|", Caret = "^",
        Tilde = "~", Bang = "!", Plus = "+", Minus = "-", Star = "*", Slash = "/", Percent = "%",
        PlusPlus = "++", Shl = "<<", Shr = ">>", PlusEq = "+=", MinusEq = "-=", StarEq = "*=",
        SlashEq = "/=", PercentEq = "%=", PlusPercentEq = "+%=", PlusPipeEq = "+|=", Underscore = "_",
    }
}

impl TokenKind {
    /// The keyword spelled `word`, if `word` is reserved.
    #[must_use]
    pub fn keyword(word: &str) -> Option<Self> {
        Self::KEYWORDS.iter().copied().find(|k| k.text() == Some(word))
    }

    #[must_use]
    pub fn is_trivia(self) -> bool {
        Self::TRIVIA.contains(&self)
    }
}

/// Where a contextual keyword is recognized; elsewhere it is an identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Context {
    /// After the pattern of a `for`.
    ForPattern,
    /// Directly after `&`.
    AfterAmp,
}

pub const CONTEXTUAL: &[(&str, Context)] =
    &[("in", Context::ForPattern), ("out", Context::AfterAmp), ("take", Context::AfterAmp)];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn spellings_are_unique() {
        let fixed = TokenKind::KEYWORDS.iter().chain(TokenKind::PUNCTUATION);
        let texts: Vec<_> = fixed.map(|k| k.text().unwrap()).collect();
        assert_eq!(texts.iter().collect::<HashSet<_>>().len(), texts.len());
    }

    #[test]
    fn keyword_lookup_round_trips() {
        for &kw in TokenKind::KEYWORDS {
            assert_eq!(TokenKind::keyword(kw.text().unwrap()), Some(kw));
        }
        assert_eq!(TokenKind::keyword("fn"), None);
    }
}
