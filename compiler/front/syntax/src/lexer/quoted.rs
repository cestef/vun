//! Strings, characters and their escapes.

use vun_diag::{ChildKind, Diagnostic, Msg, errors};
use vun_grammar::TokenKind;

use super::{
    BYTE_ESCAPE_DIGITS, ESCAPE, Lexer, SIMPLE_ESCAPES, UNICODE_ESCAPE_DIGITS, is_ident_continue,
    is_ident_start, value,
};

impl Lexer<'_, '_, '_> {
    pub(super) fn string(&mut self) -> TokenKind {
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

    /// `c"text"`: an escaped string whose value ends with a zero byte, so that it holds none
    /// before.
    pub(super) fn c_string(&mut self) -> TokenKind {
        let lo = self.pos;
        self.bump();
        let kind = self.string();
        let written = &self.text[lo..self.pos];
        if value::bytes(kind, written).is_some_and(|bytes| bytes[..bytes.len() - 1].contains(&0)) {
            let msg = Msg::new().text("a string for C holds no zero byte before its end");
            let help = Msg::new().text("use a string without ").code("c").text(" for bytes that hold one");
            let span = self.span_from(lo);
            self.cx.error_at(
                span,
                Diagnostic::error(errors::SYNTAX_INTERIOR_NUL, msg).child(ChildKind::Help, help),
            );
        }
        kind
    }

    /// `\word` or `\"raw string"`.
    pub(super) fn escaped(&mut self) -> TokenKind {
        let lo = self.pos;
        self.bump();
        match self.peek() {
            Some('"') => {
                self.bump();
                self.eat_while(|c| c != '"' && c != '\n');
                if !self.eat('"') {
                    self.unterminated(lo, errors::SYNTAX_UNTERMINATED_STRING, "raw string", '"');
                }
                TokenKind::RawString
            }
            Some(c) if is_ident_start(c) => {
                self.eat_while(is_ident_continue);
                TokenKind::RawIdent
            }
            _ => {
                let msg = Msg::new().code(ESCAPE).text(" must be followed by a word or a string");
                let help = Msg::new()
                    .text("a backslash takes the next word or string literally: ")
                    .code_list([r"\unsafe", r#"\"C:\dir""#, r"\\ raw line"]);
                let span = self.span_from(lo);
                self.cx.error_at(
                    span,
                    Diagnostic::error(errors::SYNTAX_STRAY_ESCAPE, msg).child(ChildKind::Help, help),
                );
                TokenKind::Error
            }
        }
    }

    /// A character literal `'a'` or a label `'outer`.
    pub(super) fn quote(&mut self) -> TokenKind {
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

    pub(super) fn close_char(&mut self, lo: usize) -> TokenKind {
        if !self.eat('\'') {
            self.unterminated(lo, errors::SYNTAX_UNTERMINATED_CHAR, "character", '\'');
        }
        TokenKind::Char
    }

    pub(super) fn escape(&mut self) {
        let lo = self.pos;
        self.bump();
        let valid = match self.peek() {
            Some(c) if SIMPLE_ESCAPES.iter().any(|&(e, _)| e == c) => {
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

    pub(super) fn unicode_escape(&mut self) -> bool {
        if !self.eat('{') {
            return false;
        }
        let start = self.pos;
        let digits = self.eat_hex_digits(UNICODE_ESCAPE_DIGITS);
        let value = u32::from_str_radix(&self.text[start..self.pos], 16).ok();
        self.eat('}') && digits > 0 && value.and_then(char::from_u32).is_some()
    }
}
