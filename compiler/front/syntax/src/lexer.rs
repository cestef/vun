//! Source text to tokens, recovering from errors.
//!
//! Every byte of the source ends up in exactly one lexeme. Malformed input produces a diagnostic
//! and a token that keeps the parser going: an unterminated string is still a `String`, a run of
//! unknown characters is one `Error` token.
//!
//! One rule depends on context: a number directly after `.` is lexed without a fraction, so that
//! `t.0.1` indexes twice instead of reading `0.1`.

use overt_diag::{ChildKind, DiagCx, DiagId, Diagnostic, Msg, errors};
use overt_grammar::TokenKind;
use overt_span::{ByteRange, FileId, Span};
use overt_trace::tracing;

use crate::tokens::{Builder, Lexeme, Tokens};

/// Escapes written as a backslash and one character.
const SIMPLE_ESCAPES: [char; 7] = ['n', 't', 'r', '0', '\\', '"', '\''];
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
        let lo = lexer.pos;
        let kind = lexer.lexeme();
        if !kind.is_trivia() {
            lexer.after_dot = kind == TokenKind::Dot;
        }
        out.push(Lexeme { kind, range: lexer.range_from(lo) });
    }
    let tokens = out.finish(offset(text.len()));
    tracing::debug!(tokens = tokens.len(), "lexed");
    tokens
}

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
            '"' => self.string(),
            '\'' => self.quote(),
            c if c.is_ascii_digit() => self.number(),
            c if is_ident_start(c) => self.word(),
            _ => self.punctuation(),
        }
    }

    fn comment(&mut self) -> TokenKind {
        let kind = if self.at(MODULE_DOC_COMMENT) {
            TokenKind::ModuleDocComment
        } else if self.at(DOC_COMMENT) && !self.rest()[DOC_COMMENT.len()..].starts_with('/') {
            TokenKind::DocComment
        } else {
            TokenKind::Comment
        };
        self.eat_line();
        kind
    }

    fn string(&mut self) -> TokenKind {
        let lo = self.pos;
        self.bump();
        loop {
            match self.peek() {
                None | Some('\n') => {
                    self.unterminated(lo, errors::SYNTAX_UNTERMINATED_STRING, "string", '"');
                    break;
                }
                Some('"') => {
                    self.bump();
                    break;
                }
                Some('\\') => self.escape(),
                Some(_) => self.bump(),
            }
        }
        TokenKind::String
    }

    /// A character literal `'a'` or a label `'outer`.
    fn quote(&mut self) -> TokenKind {
        let lo = self.pos;
        self.bump();
        match (self.peek(), self.peek_second()) {
            (Some('\\'), _) => {
                self.escape();
                self.close_char(lo)
            }
            (Some(c), Some('\'')) if c != '\n' => {
                self.bump();
                self.bump();
                TokenKind::Char
            }
            (Some(c), _) if is_ident_start(c) => {
                self.eat_while(is_ident_continue);
                TokenKind::Label
            }
            (next, _) => {
                if next.is_some_and(|c| c != '\n') {
                    self.bump();
                }
                self.close_char(lo)
            }
        }
    }

    fn close_char(&mut self, lo: usize) -> TokenKind {
        if !self.eat('\'') {
            self.unterminated(lo, errors::SYNTAX_UNTERMINATED_CHAR, "character", '\'');
        }
        TokenKind::Char
    }

    fn escape(&mut self) {
        let lo = self.pos;
        self.bump();
        let valid = match self.peek() {
            Some(c) if SIMPLE_ESCAPES.contains(&c) => {
                self.bump();
                true
            }
            Some('x') => {
                self.bump();
                self.eat_hex_digits(BYTE_ESCAPE_DIGITS) == BYTE_ESCAPE_DIGITS
            }
            Some('u') => {
                self.bump();
                self.unicode_escape()
            }
            None | Some('\n') => false,
            Some(_) => {
                self.bump();
                false
            }
        };
        if !valid {
            self.invalid_escape(lo);
        }
    }

    fn unicode_escape(&mut self) -> bool {
        if !self.eat('{') {
            return false;
        }
        let start = self.pos;
        let digits = self.eat_hex_digits(UNICODE_ESCAPE_DIGITS);
        let value = u32::from_str_radix(&self.text[start..self.pos], 16).ok();
        self.eat('}') && digits > 0 && value.and_then(char::from_u32).is_some()
    }

    fn number(&mut self) -> TokenKind {
        let lo = self.pos;
        let prefix = match (self.peek(), self.peek_second()) {
            (Some('0'), Some(p)) => RADIX_PREFIXES.iter().find(|&&(c, _)| c == p),
            _ => None,
        };
        let mut kind = TokenKind::Int;
        let mut well_formed = true;
        if let Some(&(_, radix)) = prefix {
            self.bump();
            self.bump();
            well_formed = self.eat_digits(radix) > 0;
        } else {
            self.eat_digits(10);
            if !self.after_dot && self.fraction_follows() {
                self.bump();
                self.eat_digits(10);
                kind = TokenKind::Float;
            }
            if !self.after_dot && self.exponent_follows() {
                self.bump();
                if matches!(self.peek(), Some('+' | '-')) {
                    self.bump();
                }
                self.eat_digits(10);
                kind = TokenKind::Float;
            }
        }
        // Letters or digits glued to a literal make it malformed: `0b102`, `12ab`, `0x`.
        if self.peek().is_some_and(is_ident_continue) {
            self.eat_while(is_ident_continue);
            well_formed = false;
        }
        if !well_formed {
            let msg = Msg::new().text("malformed number literal ").code(&self.text[lo..self.pos]);
            self.error(lo, errors::SYNTAX_MALFORMED_NUMBER, msg);
        }
        kind
    }

    fn fraction_follows(&self) -> bool {
        self.peek() == Some('.') && self.peek_second().is_some_and(|c| c.is_ascii_digit())
    }

    fn exponent_follows(&self) -> bool {
        let mut rest = self.rest().chars();
        matches!(rest.next(), Some('e' | 'E')) && {
            let after = rest.next();
            let digit = if matches!(after, Some('+' | '-')) { rest.next() } else { after };
            digit.is_some_and(|c| c.is_ascii_digit())
        }
    }

    fn word(&mut self) -> TokenKind {
        let lo = self.pos;
        self.eat_while(is_ident_continue);
        let word = &self.text[lo..self.pos];
        TokenKind::keyword(word).unwrap_or(if TokenKind::Underscore.text() == Some(word) {
            TokenKind::Underscore
        } else {
            TokenKind::Ident
        })
    }

    fn punctuation(&mut self) -> TokenKind {
        if let Some(kind) = TokenKind::punctuation(self.rest()) {
            self.pos += kind.text().map_or(0, str::len);
            return kind;
        }
        let lo = self.pos;
        self.bump();
        while self.pos < self.text.len() && !self.starts_token() {
            self.bump();
        }
        let msg = Msg::new().text("not a valid token: ").code(&self.text[lo..self.pos]);
        self.error(lo, errors::SYNTAX_UNKNOWN_CHARACTER, msg);
        TokenKind::Error
    }

    fn starts_token(&self) -> bool {
        self.peek().is_some_and(|c| {
            c.is_ascii_whitespace() || is_ident_start(c) || c.is_ascii_digit() || c == '"' || c == '\''
        }) || self.at(RAW_STRING_LINE)
            || TokenKind::punctuation(self.rest()).is_some()
    }

    fn unterminated(&mut self, lo: usize, id: DiagId, what: &'static str, close: char) {
        let msg = Msg::new().text("unterminated ").text(what).text(" literal");
        let help = Msg::new().text("add a closing ").code(close);
        let span = self.span_from(lo);
        self.cx.error_at(span, Diagnostic::error(id, msg).child(ChildKind::Help, help));
    }

    fn invalid_escape(&mut self, lo: usize) {
        let msg = Msg::new().text("invalid escape sequence ").code(&self.text[lo..self.pos]);
        let simple = SIMPLE_ESCAPES.iter().map(|&c| ['\\', c].iter().collect::<String>());
        let valid = simple.chain(["\\xHH".to_owned(), "\\u{H..HHHHHH}".to_owned()]);
        let note = Msg::new().text("valid escapes are ").code_list(valid);
        let span = self.span_from(lo);
        let d = Diagnostic::error(errors::SYNTAX_INVALID_ESCAPE, msg).child(ChildKind::Note, note);
        self.cx.error_at(span, d);
    }

    fn error(&mut self, lo: usize, id: DiagId, msg: Msg) {
        let span = self.span_from(lo);
        self.cx.error_at(span, Diagnostic::error(id, msg));
    }

    fn rest(&self) -> &str {
        &self.text[self.pos..]
    }

    fn at(&self, prefix: &str) -> bool {
        self.rest().starts_with(prefix)
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn peek_second(&self) -> Option<char> {
        self.rest().chars().nth(1)
    }

    fn bump(&mut self) {
        if let Some(c) = self.peek() {
            self.pos += c.len_utf8();
        }
    }

    fn eat(&mut self, c: char) -> bool {
        let found = self.peek() == Some(c);
        if found {
            self.pos += c.len_utf8();
        }
        found
    }

    fn eat_while(&mut self, mut accept: impl FnMut(char) -> bool) {
        while let Some(c) = self.peek().filter(|&c| accept(c)) {
            self.pos += c.len_utf8();
        }
    }

    fn eat_line(&mut self) {
        self.eat_while(|c| c != '\n');
    }

    /// Digits of `radix` with `_` separators; returns the number of digits.
    fn eat_digits(&mut self, radix: u32) -> usize {
        let mut digits = 0;
        self.eat_while(|c| {
            digits += usize::from(c.is_digit(radix));
            c.is_digit(radix) || c == '_'
        });
        digits
    }

    /// Up to `limit` hex digits, without separators; returns how many were read.
    fn eat_hex_digits(&mut self, limit: usize) -> usize {
        let mut digits = 0;
        self.eat_while(|c| {
            let take = digits < limit && c.is_ascii_hexdigit();
            digits += usize::from(take);
            take
        });
        digits
    }

    fn range_from(&self, lo: usize) -> ByteRange {
        ByteRange::new(offset(lo), offset(self.pos))
    }

    fn span_from(&self, lo: usize) -> Span {
        Span { file: self.file, range: self.range_from(lo) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use TokenKind::*;
    use overt_diag::DiagCtxt;
    use overt_origin::OriginTable;
    use overt_span::SourceFiles;

    fn lex_text(text: &str) -> (Vec<(TokenKind, std::string::String)>, Vec<DiagId>) {
        let file = SourceFiles::new().add("t.ov", text);
        let (mut diag, mut origins) = (DiagCtxt::new(), OriginTable::new());
        let tokens = lex(file, text, &mut DiagCx { diag: &mut diag, origins: &mut origins });
        let significant = tokens.ids().map(|t| (tokens.kind(t), tokens.range(t).of(text).to_owned()));
        let ids = diag.sorted().into_iter().map(|d| d.id.clone()).collect();
        (significant.collect(), ids)
    }

    fn kinds(text: &str) -> Vec<TokenKind> {
        let (tokens, ids) = lex_text(text);
        assert!(ids.is_empty(), "unexpected diagnostics {ids:?} for {text:?}");
        tokens.into_iter().map(|(k, _)| k).collect()
    }

    #[test]
    fn words() {
        assert_eq!(kinds("mut x := _y _ for"), [Mut, Ident, ColonEq, Ident, Underscore, For, Eof]);
    }

    #[test]
    fn numbers() {
        assert_eq!(
            kinds("1_000 0xff 0o17 0b1010 1.5 1e-3 2E+8"),
            [Int, Int, Int, Int, Float, Float, Float, Eof]
        );
        assert_eq!(kinds("0..n"), [Int, DotDot, Ident, Eof]);
        assert_eq!(kinds("t.0.1"), [Ident, Dot, Int, Dot, Int, Eof]);
        assert_eq!(kinds("x.e1"), [Ident, Dot, Ident, Eof]);
    }

    #[test]
    fn quotes() {
        assert_eq!(
            kinds(r#""a\n\x41\u{1F600}" 'x' '\'' 'é' 'outer:"#),
            [String, Char, Char, Char, Label, Colon, Eof]
        );
        assert_eq!(kinds("\\\\ raw line\nx"), [RawStringLine, Ident, Eof]);
    }

    #[test]
    fn comments_are_trivia_and_docs_are_not() {
        assert_eq!(
            kinds("/// doc\n//! module\n// plain\n//// plain\nx"),
            [DocComment, ModuleDocComment, Ident, Eof]
        );
    }

    #[test]
    fn punctuation_prefers_longest() {
        assert_eq!(
            kinds("x +%= 1; a..=b; p.*; #{ $x }"),
            [
                Ident,
                PlusPercentEq,
                Int,
                Semi,
                Ident,
                DotDotEq,
                Ident,
                Semi,
                Ident,
                DotStar,
                Semi,
                HashLBrace,
                Dollar,
                Ident,
                RBrace,
                Eof
            ]
        );
    }

    #[test]
    fn errors_are_reported_once_and_lexing_continues() {
        let cases = [
            ("\"open", errors::SYNTAX_UNTERMINATED_STRING, vec![String, Eof]),
            ("'\\n", errors::SYNTAX_UNTERMINATED_CHAR, vec![Char, Eof]),
            (r#""\q" x"#, errors::SYNTAX_INVALID_ESCAPE, vec![String, Ident, Eof]),
            ("0b102 x", errors::SYNTAX_MALFORMED_NUMBER, vec![Int, Ident, Eof]),
            ("0x", errors::SYNTAX_MALFORMED_NUMBER, vec![Int, Eof]),
            ("@@ x", errors::SYNTAX_UNKNOWN_CHARACTER, vec![Error, Ident, Eof]),
        ];
        for (text, id, expected) in cases {
            let (tokens, ids) = lex_text(text);
            assert_eq!(ids, [id], "{text:?}");
            assert_eq!(tokens.into_iter().map(|(k, _)| k).collect::<Vec<_>>(), expected, "{text:?}");
        }
    }
}
