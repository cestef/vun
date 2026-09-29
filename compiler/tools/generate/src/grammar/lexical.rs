//! Pieces of the tokens the grammar has no table of, since the lexer reads them by hand, for the
//! editor grammars: each is composed from the lexer's constants, and what the lexer matches as a
//! bare character is named here once.

use vun_syntax::lexer::{BYTE_ESCAPE_DIGITS, DIGIT_SEPARATOR, ESCAPE, SIMPLE_ESCAPES, UNICODE_ESCAPE_DIGITS};

use super::pattern::Pattern;

/// Quotes a string.
pub(super) const QUOTE: char = '"';
/// Quotes a character, and starts a label.
pub(super) const APOSTROPHE: char = '\'';
/// Ends every token but a whole line.
pub(super) const LINE_END: char = '\n';
/// Starts a byte escape `\xHH` and a Unicode escape `\u{H}`.
pub(super) const BYTE_ESCAPE: char = 'x';
pub(super) const UNICODE_ESCAPE: char = 'u';
/// Around the digits of a Unicode escape.
pub(super) const UNICODE_OPEN: char = '{';
pub(super) const UNICODE_CLOSE: char = '}';
/// A number starting with it may have a radix prefix.
pub(super) const RADIX_LEAD: char = '0';
/// Starts the exponent of a float, then its optional sign.
pub(super) const EXPONENT: [char; 2] = ['e', 'E'];
pub(super) const SIGNS: [char; 2] = ['+', '-'];
/// Separates a float's fraction, and a member.
pub(super) const DOT: char = '.';
pub(super) const DECIMAL: u32 = 10;
pub(super) const HEX: u32 = 16;
/// The lexer's `is_ident_start` and `is_ident_continue`.
pub(super) const IDENT: &str = "[A-Za-z_][A-Za-z0-9_]*";
/// A name starting with a capital, which the naming lints keep for types.
pub(super) const TYPE_NAME: &str = "[A-Z][A-Za-z0-9_]*";

/// The digits of `radix`, letters in either case.
pub(super) fn digit_chars(radix: u32) -> impl Iterator<Item = char> {
    let lower = (0..radix).filter_map(move |d| char::from_digit(d, radix));
    lower.clone().chain(lower.filter(char::is_ascii_alphabetic).map(|c| c.to_ascii_uppercase()))
}

/// The digits of `radix`, then the separator.
pub(super) fn digits(radix: u32) -> Pattern {
    Pattern::class(digit_chars(radix).chain([DIGIT_SEPARATOR]))
}

pub(super) fn char_of(c: char) -> Pattern {
    Pattern::literal(&c.to_string())
}

/// A valid escape inside a string or character.
pub(super) fn escape() -> Pattern {
    let hex = Pattern::class(digit_chars(HEX));
    let simple = Pattern::class(SIMPLE_ESCAPES.iter().map(|&(c, _)| c));
    let byte = char_of(BYTE_ESCAPE).then(hex.clone().times(BYTE_ESCAPE_DIGITS, BYTE_ESCAPE_DIGITS));
    let unicode = char_of(UNICODE_ESCAPE)
        .then(char_of(UNICODE_OPEN))
        .then(hex.times(1, UNICODE_ESCAPE_DIGITS))
        .then(char_of(UNICODE_CLOSE));
    char_of(ESCAPE).then(Pattern::any([simple, byte, unicode]))
}
