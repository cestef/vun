//! Source text to tokens, recovering from errors.
//!
//! Every byte of the source ends up in exactly one lexeme. Malformed input produces a diagnostic
//! and a token that keeps the parser going: an unterminated string is still a `String`, a run of
//! unknown characters is one `Error` token.
//!
//! A backslash before a token takes it literally: `\unsafe` is a name although `unsafe` is a
//! keyword, `\"C:\dir"` is a string without escapes, and `\\` starts a raw line.
//!
//! One rule depends on context: a number directly after `.` is lexed without a fraction, so that
//! `t.0.1` indexes twice instead of reading `0.1`.

use vun_diag::{DiagCtxt, DiagCx};
use vun_grammar::TokenKind;
use vun_origin::OriginTable;
use vun_span::FileId;
use vun_trace::tracing;

use crate::tokens::{Builder, Lexeme, Tokens};

mod cursor;
mod number;
mod quoted;
mod report;
#[cfg(test)]
mod tests;
mod value;
mod words;

pub use value::{bytes, literal, scalar};

/// Escapes written as a backslash and one character, with the character each denotes.
pub const SIMPLE_ESCAPES: [(char, char); 7] =
    [('n', '\n'), ('t', '\t'), ('r', '\r'), ('0', '\0'), ('\\', '\\'), ('"', '"'), ('\'', '\'')];
/// Hex digits in a byte escape `\xHH`.
pub const BYTE_ESCAPE_DIGITS: usize = 2;
/// Maximum hex digits in a Unicode escape `\u{HHHHHH}`.
pub const UNICODE_ESCAPE_DIGITS: usize = 6;
/// Integer prefixes after a leading `0`, with their radix.
pub const RADIX_PREFIXES: [(char, u32); 3] = [('x', 16), ('o', 8), ('b', 2)];
/// What may stand between the digits of a number: `1_000`.
pub const DIGIT_SEPARATOR: char = '_';

pub const COMMENT: &str = "//";
pub const DOC_COMMENT: &str = "///";
pub const MODULE_DOC_COMMENT: &str = "//!";
pub const RAW_STRING_LINE: &str = "\\\\";

/// Tokens that run to the end of their line, which ends them: doc comments and raw string lines.
pub const TO_LINE_END: &[TokenKind] =
    &[TokenKind::DocComment, TokenKind::ModuleDocComment, TokenKind::RawStringLine];

/// Takes the next word or string literally.
pub const ESCAPE: char = '\\';

impl Tokens {
    /// Whether the token text `next`, written right after the source `before` in `file`, lexes
    /// as one token starting where `before` ends: after `2` and `.`, `5` would make `2.5`, and
    /// after `..`, `=` would make `..=`. A line of source lexes alone, so `before` is the line
    /// written so far.
    #[must_use]
    pub fn apart(file: FileId, before: &str, next: &str) -> bool {
        let text = [before, next].concat();
        let mut diag = DiagCtxt::new();
        let tokens = Self::lex(file, &text, &mut DiagCx { diag: &mut diag, origins: OriginTable::global() });
        let significant = tokens.ids().filter(|&t| tokens.kind(t) != TokenKind::Eof);
        let last = significant.last().map(|t| tokens.range(t));
        diag.error_count() == 0
            && last.is_some_and(|r| r.lo as usize == before.len() && r.hi as usize == text.len())
    }

    /// Lexes one file. Diagnostics go to `cx`; the result is always a complete token stream.
    ///
    /// # Panics
    /// Panics if `text` is 4 GiB or larger, which `SourceFile` already rules out.
    pub fn lex(file: FileId, text: &str, cx: &mut DiagCx<'_>) -> Self {
        let _span = tracing::debug_span!("lex", ?file, bytes = text.len()).entered();
        assert!(u32::try_from(text.len()).is_ok(), "source files are smaller than 4 GiB");
        let mut lexer = Lexer { file, text, pos: 0, after_dot: false, cx };
        let mut out = Builder::new(file, text.len() / 4);
        while lexer.pos < text.len() {
            let (lo, reported) = (lexer.pos, lexer.cx.diag.error_count());
            let kind = lexer.lexeme();
            if !kind.is_trivia() {
                lexer.after_dot = kind == TokenKind::Dot;
            }
            out.push(Lexeme { kind, range: lexer.range_from(lo) });
            if lexer.cx.diag.error_count() > reported {
                out.diagnosed();
            }
        }
        let tokens = out.finish(offset(text.len()));
        tracing::debug!(tokens = tokens.len(), "lexed");
        tokens
    }
}

/// The text a string, raw string, raw line or character token denotes; `None` for other tokens
/// and for malformed literals, which the lexer reported.
fn offset(pos: usize) -> u32 {
    u32::try_from(pos).expect("offsets fit u32, checked in lex")
}

pub(crate) fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

pub(crate) fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Whether `word` lexes as one name.
pub(crate) fn is_ident(word: &str) -> bool {
    let mut chars = word.chars();
    chars.next().is_some_and(is_ident_start) && chars.all(is_ident_continue)
}

struct Lexer<'s, 'c, 'd> {
    file: FileId,
    text: &'s str,
    pos: usize,
    after_dot: bool,
    cx: &'c mut DiagCx<'d>,
}

impl Lexer<'_, '_, '_> {
    fn lexeme(&mut self) -> TokenKind {
        match self.peek().expect("lexeme called at end of input") {
            c if c.is_ascii_whitespace() => {
                self.eat_while(|c| c.is_ascii_whitespace());
                TokenKind::Whitespace
            }
            _ if self.at(COMMENT) => self.comment(),
            _ if self.at(RAW_STRING_LINE) => {
                self.eat_line();
                TokenKind::RawStringLine
            }
            ESCAPE => self.escaped(),
            '"' => self.string(),
            '\'' => self.quote(),
            c if c.is_ascii_digit() => self.number(),
            c if is_ident_start(c) => self.word(),
            _ => self.punctuation(),
        }
    }
}
