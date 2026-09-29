//! The classes tokens have by their kind alone, or by the node of the syntax tree they anchor.

use vun_grammar::{ASSIGN, NodeKind, POSTFIX, PREFIX, TokenKind};
use vun_hir::db::ParsedFile;
use vun_index::IndexVec;
use vun_syntax::cst::Anchor;
use vun_syntax::tokens::{Lexeme, TokenId, Tokens};

use super::{Class, Modifier, Modifiers};

/// Every lexeme of `tokens` in order, with its token when it is one rather than trivia.
pub(super) fn lexemes(tokens: &Tokens) -> impl Iterator<Item = (Lexeme, Option<TokenId>)> + '_ {
    tokens.ids().flat_map(move |t| {
        let token = Lexeme { kind: tokens.kind(t), range: tokens.range(t) };
        tokens.leading_trivia(t).map(|l| (l, None)).chain(std::iter::once((token, Some(t))))
    })
}

/// The class of a trivia run.
pub(super) fn trivia(kind: TokenKind) -> Option<(Class, Modifiers)> {
    (kind == TokenKind::Comment).then_some((Class::Comment, Modifiers::default()))
}

/// The tokens of a file that are words only where they stand: a borrow mode, an operand's flow,
/// the `in` of a `for`, the `asm` of an assembly block, an instruction's mnemonic.
pub(super) struct Contextual(IndexVec<TokenId, bool>);

impl Contextual {
    pub(super) fn of(parsed: &ParsedFile) -> Self {
        let (cst, tokens) = (&parsed.cst, &parsed.tokens);
        let mut words = IndexVec::filled(false, tokens.len());
        for n in cst.ids() {
            let word = matches!(
                cst.kind(n),
                NodeKind::Qualifier
                    | NodeKind::Mode
                    | NodeKind::Flow
                    | NodeKind::In
                    | NodeKind::Punct
                    | NodeKind::Line
            );
            if let Anchor::Token(t) = cst.anchor(n)
                && word
                && matches!(tokens.kind(t), TokenKind::Ident | TokenKind::RawIdent)
            {
                words[t] = true;
            }
        }
        Self(words)
    }

    /// The class of the token `t` by its kind, or as a contextual word; `None` for a name, which
    /// resolution classes, and for punctuation that only delimits.
    pub(super) fn token(&self, tokens: &Tokens, t: TokenId) -> Option<(Class, Modifiers)> {
        let plain = |class| Some((class, Modifiers::default()));
        let kind = tokens.kind(t);
        match kind {
            TokenKind::DocComment | TokenKind::ModuleDocComment => {
                Some((Class::Comment, Modifiers::default().with(Modifier::Documentation)))
            }
            TokenKind::Int | TokenKind::Float => plain(Class::Number),
            TokenKind::Char | TokenKind::String | TokenKind::RawString | TokenKind::RawStringLine => {
                plain(Class::String)
            }
            TokenKind::Label => plain(Class::Label),
            TokenKind::HashLBracket => plain(Class::Attribute),
            TokenKind::Ident | TokenKind::RawIdent => {
                self.0[t].then_some((Class::Keyword, Modifiers::default()))
            }
            _ if kind.is_keyword() => plain(Class::Keyword),
            _ if kind.infix().is_some()
                || [PREFIX, POSTFIX, ASSIGN].iter().any(|table| table.contains(&kind)) =>
            {
                plain(Class::Operator)
            }
            _ => None,
        }
    }
}
