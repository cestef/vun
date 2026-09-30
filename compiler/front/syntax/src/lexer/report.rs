//! Lexer diagnostics.

use vun_diag::{DiagId, Diagnostic, Fix, Msg, Safety, errors};
use vun_span::Span;

use super::{Lexer, SIMPLE_ESCAPES};

/// What makes a number literal malformed.
#[derive(Clone, Copy)]
pub(super) enum Malformed {
    /// A base's prefix without a digit after it: `0x`.
    Bare,
    /// Letters or digits of no digit of the number's base, glued to it from `at` on: `0b12`,
    /// `12ab`.
    Glued { at: usize, radix: u32 },
}

impl Lexer<'_, '_, '_> {
    pub(super) fn unterminated(&mut self, lo: usize, id: DiagId, what: &'static str, close: char) {
        let msg = Msg::new().text("unterminated ").text(what).text(" literal");
        let help = Msg::new().text("add a closing ").code(close);
        let span = self.span_from(lo);
        let end = Span { file: self.file, range: self.range_from(self.pos) };
        let fix = Fix::insert(help, Safety::Unsafe, end, close).preferred();
        self.cx.error_at(span, Diagnostic::error(id, msg).fix(fix));
    }

    pub(super) fn invalid_escape(&mut self, lo: usize) {
        let msg = Msg::new().text("invalid escape sequence ").code(&self.text[lo..self.pos]);
        let simple = SIMPLE_ESCAPES.iter().map(|&(c, _)| ['\\', c].iter().collect::<String>());
        let valid = simple.chain(["\\xHH".to_owned(), "\\u{H..HHHHHH}".to_owned()]);
        let note = Msg::new().text("valid escapes are ").code_list(valid);
        let span = self.span_from(lo);
        // The backslash alone, written as the escape of a backslash.
        let backslash =
            Span { file: self.file, range: vun_span::ByteRange::new(span.range.lo, span.range.lo + 1) };
        let help = Msg::new().text("to write a backslash, escape it: ").code("\\\\");
        let fix = Fix::replace(help, Safety::Unsafe, backslash, "\\\\");
        let d = Diagnostic::error(errors::SYNTAX_INVALID_ESCAPE, msg).note(note).fix(fix);
        self.cx.error_at(span, d);
    }

    /// Reports the number literal from `lo` to here, malformed as `malformed` says.
    pub(super) fn malformed(&mut self, lo: usize, malformed: Malformed) {
        let literal = &self.text[lo..self.pos];
        let d = match malformed {
            Malformed::Bare => {
                let msg = Msg::new().code(literal).text(" has no digits");
                let help = Msg::new().text("write its digits after ").code(literal);
                let at = self.span_from(self.pos);
                let fix = Fix::insert(help, Safety::Placeholder, at, "0");
                Diagnostic::error(errors::SYNTAX_MALFORMED_NUMBER, msg).fix(fix)
            }
            Malformed::Glued { at, radix } => {
                let glued = &self.text[at..self.pos];
                let first = glued.chars().next().unwrap_or_default();
                if first.is_ascii_digit() || (radix > 10 && first.is_ascii_hexdigit()) {
                    let msg = Msg::new()
                        .code(first)
                        .text(" is not a digit of a ")
                        .text(Malformed::base(radix))
                        .text(" number");
                    let last = char::from_digit(radix - 1, radix).unwrap_or_default();
                    let note = Msg::new().text("its digits are ").code('0').text(" to ").code(last);
                    let help = Msg::new().text("remove it, or write the number in a base that has it");
                    Diagnostic::error(errors::SYNTAX_MALFORMED_NUMBER, msg).note(note).help(help)
                } else {
                    let number = &self.text[lo..at];
                    let msg = Msg::new().code(glued).text(" is written against the number ").code(number);
                    let help = Msg::new().text("separate them with a space");
                    let space = Span { file: self.file, range: self.range_from(at) };
                    let fix = Fix::insert(help, Safety::Unsafe, space, " ").preferred();
                    Diagnostic::error(errors::SYNTAX_MALFORMED_NUMBER, msg).fix(fix)
                }
            }
        };
        let span = self.span_from(lo);
        self.cx.error_at(span, d);
    }

    /// Reports the text from `lo` to here, which starts no token.
    pub(super) fn unknown(&mut self, lo: usize) {
        let msg = Msg::new().code(&self.text[lo..self.pos]).text(" is not part of Vun's syntax");
        let span = self.span_from(lo);
        let fix = Fix::remove(Msg::new().text("remove it"), Safety::Unsafe, span).preferred();
        self.cx.error_at(span, Diagnostic::error(errors::SYNTAX_UNKNOWN_CHARACTER, msg).fix(fix));
    }
}

impl Malformed {
    /// The name of the base `radix`, as a number written in it is called.
    const fn base(radix: u32) -> &'static str {
        match radix {
            2 => "binary",
            8 => "octal",
            16 => "hexadecimal",
            _ => "decimal",
        }
    }
}
