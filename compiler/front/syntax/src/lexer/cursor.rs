//! The character cursor over the source text.

use vun_span::{ByteRange, Span};

use super::{Lexer, offset};

impl Lexer<'_, '_, '_> {
    pub(super) fn rest(&self) -> &str {
        &self.text[self.pos..]
    }

    pub(super) fn at(&self, prefix: &str) -> bool {
        self.rest().starts_with(prefix)
    }

    pub(super) fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    pub(super) fn peek_second(&self) -> Option<char> {
        self.rest().chars().nth(1)
    }

    pub(super) fn bump(&mut self) {
        if let Some(c) = self.peek() {
            self.pos += c.len_utf8();
        }
    }

    pub(super) fn eat(&mut self, c: char) -> bool {
        let found = self.peek() == Some(c);
        if found {
            self.pos += c.len_utf8();
        }
        found
    }

    pub(super) fn eat_while(&mut self, mut accept: impl FnMut(char) -> bool) {
        while let Some(c) = self.peek().filter(|&c| accept(c)) {
            self.pos += c.len_utf8();
        }
    }

    pub(super) fn eat_line(&mut self) {
        self.eat_while(|c| c != '\n');
    }

    /// Digits of `radix` with `_` separators; returns the number of digits.
    pub(super) fn eat_digits(&mut self, radix: u32) -> usize {
        let mut digits = 0;
        self.eat_while(|c| {
            digits += usize::from(c.is_digit(radix));
            c.is_digit(radix) || c == '_'
        });
        digits
    }

    /// Up to `limit` hex digits, without separators; returns how many were read.
    pub(super) fn eat_hex_digits(&mut self, limit: usize) -> usize {
        let mut digits = 0;
        self.eat_while(|c| {
            let take = digits < limit && c.is_ascii_hexdigit();
            digits += usize::from(take);
            take
        });
        digits
    }

    pub(super) fn range_from(&self, lo: usize) -> ByteRange {
        ByteRange::new(offset(lo), offset(self.pos))
    }

    pub(super) fn span_from(&self, lo: usize) -> Span {
        Span { file: self.file, range: self.range_from(lo) }
    }
}
