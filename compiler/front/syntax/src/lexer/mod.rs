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

use overt_diag::DiagCx;
use overt_grammar::TokenKind;
use overt_span::FileId;
use overt_trace::tracing;

use crate::tokens::{Builder, Lexeme, Tokens};

mod cursor;
mod number;
mod quoted;
mod report;
#[cfg(test)]
mod tests;
mod value;
mod words;

pub use value::literal;

/// Escapes written as a backslash and one character, with the character each denotes.
const SIMPLE_ESCAPES: [(char, char); 7] =
    [('n', '\n'), ('t', '\t'), ('r', '\r'), ('0', '\0'), ('\\', '\\'), ('"', '"'), ('\'', '\'')];
/// Hex digits in a byte escape `\xHH`.
const BYTE_ESCAPE_DIGITS: usize = 2;
/// Maximum hex digits in a Unicode escape `\u{HHHHHH}`.
const UNICODE_ESCAPE_DIGITS: usize = 6;
/// Integer prefixes after a leading `0`, with their radix.
const RADIX_PREFIXES: [(char, u32); 3] = [('x', 16), ('o', 8), ('b', 2)];

const COMMENT: &str = "//";
const DOC_COMMENT: &str = "///";
const MODULE_DOC_COMMENT: &str = "//!";
const RAW_STRING_LINE: &str = "\\\\";
/// Takes the next word or string literally.
const ESCAPE: char = '\\';

/// Lexes one file. Diagnostics go to `cx`; the result is always a complete token stream.
///
/// # Panics
/// Panics if `text` is 4 GiB or larger, which `SourceFile` already rules out.
pub fn lex(file: FileId, text: &str, cx: &mut DiagCx<'_>) -> Tokens {
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

/// The text a string, raw string, raw line or character token denotes; `None` for other tokens
/// and for malformed literals, which the lexer reported.
fn offset(pos: usize) -> u32 {
    u32::try_from(pos).expect("offsets fit u32, checked in lex")
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
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
