//! Rules for the tokens the grammar has no table of, since the lexer reads them by hand: comments,
//! strings, characters, numbers, labels and escaped names. Each is composed from the lexer's
//! constants; what the lexer matches as a bare character is named here once.

use vun_syntax::lexer::{
    BYTE_ESCAPE_DIGITS, COMMENT, DIGIT_SEPARATOR, DOC_COMMENT, ESCAPE, MODULE_DOC_COMMENT, RADIX_PREFIXES,
    RAW_STRING_LINE, SIMPLE_ESCAPES, UNICODE_ESCAPE_DIGITS,
};

use super::pattern::Pattern;
use super::syntax::{Context, Rule, Scope, Then};

/// Quotes a string.
const QUOTE: char = '"';
/// Quotes a character, and starts a label.
const APOSTROPHE: char = '\'';
/// Ends every token but a whole line.
const LINE_END: char = '\n';
/// Starts a byte escape `\xHH` and a Unicode escape `\u{H}`.
const BYTE_ESCAPE: char = 'x';
const UNICODE_ESCAPE: char = 'u';
/// Around the digits of a Unicode escape.
const UNICODE_OPEN: char = '{';
const UNICODE_CLOSE: char = '}';
/// A number starting with it may have a radix prefix.
const RADIX_LEAD: char = '0';
/// Starts the exponent of a float, then its optional sign.
const EXPONENT: [char; 2] = ['e', 'E'];
const SIGNS: [char; 2] = ['+', '-'];
/// Separates a float's fraction, and a member.
const DOT: char = '.';
const DECIMAL: u32 = 10;
const HEX: u32 = 16;
/// The lexer's `is_ident_start` and `is_ident_continue`.
pub(super) const IDENT: &str = "[A-Za-z_][A-Za-z0-9_]*";
/// A name starting with a capital, which the naming lints keep for types.
pub(super) const TYPE_NAME: &str = "[A-Z][A-Za-z0-9_]*";

/// The digits of `radix`, letters in either case.
fn digit_chars(radix: u32) -> impl Iterator<Item = char> {
    let lower = (0..radix).filter_map(move |d| char::from_digit(d, radix));
    lower.clone().chain(lower.filter(char::is_ascii_alphabetic).map(|c| c.to_ascii_uppercase()))
}

/// The digits of `radix`, then the separator.
fn digits(radix: u32) -> Pattern {
    Pattern::class(digit_chars(radix).chain([DIGIT_SEPARATOR]))
}

fn char_of(c: char) -> Pattern {
    Pattern::literal(&c.to_string())
}

/// A valid escape inside a string or character.
fn escape() -> Pattern {
    let hex = Pattern::class(digit_chars(HEX));
    let simple = Pattern::class(SIMPLE_ESCAPES.iter().map(|&(c, _)| c));
    let byte = char_of(BYTE_ESCAPE).then(hex.clone().times(BYTE_ESCAPE_DIGITS, BYTE_ESCAPE_DIGITS));
    let unicode = char_of(UNICODE_ESCAPE)
        .then(char_of(UNICODE_OPEN))
        .then(hex.times(1, UNICODE_ESCAPE_DIGITS))
        .then(char_of(UNICODE_CLOSE));
    char_of(ESCAPE).then(Pattern::any([simple, byte, unicode]))
}

pub(super) fn comments() -> Vec<Rule> {
    // A doc comment's mark doubled is a plain comment again: `////`.
    let mark = DOC_COMMENT.strip_prefix(COMMENT).expect("a doc comment starts as a comment");
    let doc = Pattern::literal(DOC_COMMENT).not_before(&Pattern::literal(mark));
    vec![
        Rule::of(Pattern::literal(MODULE_DOC_COMMENT).through_line_end(), Scope::DocComment),
        Rule::of(doc.through_line_end(), Scope::DocComment),
        Rule::of(Pattern::literal(COMMENT).through_line_end(), Scope::Comment),
    ]
}

/// The rules starting strings and characters, and the contexts inside strings.
pub(super) fn strings() -> Vec<(Context, Vec<Rule>)> {
    let quote = char_of(QUOTE);
    let plain = Pattern::except([APOSTROPHE, ESCAPE, LINE_END]);
    let character = char_of(APOSTROPHE).then(Pattern::any([escape(), plain])).then(char_of(APOSTROPHE));
    let starts = vec![
        Rule::of(Pattern::literal(RAW_STRING_LINE).through_line_end(), Scope::RawLine),
        Rule::of(char_of(ESCAPE).then(quote.clone()), Scope::StringBegin)
            .then(Then::Push(Context::RawString)),
        Rule::of(quote.clone(), Scope::StringBegin).then(Then::Push(Context::String)),
        Rule::of(character, Scope::Char),
    ];
    // Both end at their line's end, where the lexer reports them unterminated.
    let end = || Rule::of(quote.clone(), Scope::StringEnd).then(Then::Pop);
    let unterminated = || Rule::of(Pattern::line_end(), Scope::String).then(Then::Pop);
    let string = vec![
        Rule::MetaScope(Scope::String),
        Rule::of(escape(), Scope::Escape),
        Rule::of(char_of(ESCAPE).then(Pattern::raw(".")), Scope::InvalidEscape),
        end(),
        unterminated(),
    ];
    let raw = vec![Rule::MetaScope(Scope::RawString), end(), unterminated()];
    vec![(Context::Strings, starts), (Context::String, string), (Context::RawString, raw)]
}

pub(super) fn literals() -> Vec<Rule> {
    let decimal = digits(DECIMAL);
    let digit = Pattern::raw("[0-9]");
    let run = digit.clone().then(decimal.clone().many());
    let prefixed = RADIX_PREFIXES.iter().map(|&(prefix, radix)| {
        char_of(RADIX_LEAD).then(char_of(prefix)).then(digits(radix)).then(digits(radix).many())
    });
    let fraction = char_of(DOT).then(run.clone());
    let exponent = Pattern::class(EXPONENT).then(Pattern::class(SIGNS).optional()).then(run.clone());
    vec![
        // After a `.`, a number is a member index without a fraction: `t.0.1`.
        Rule::of(run.clone().word().after(&[char_of(DOT)]), Scope::Integer),
        Rule::of(Pattern::any(prefixed).word(), Scope::Integer),
        Rule::of(run.then(fraction.optional()).then(exponent.optional()).word(), Scope::Number),
    ]
}

/// Escaped names and labels.
pub(super) fn names() -> Vec<Rule> {
    vec![
        Rule::of(char_of(ESCAPE).then(Pattern::raw(IDENT)), Scope::Escaped),
        Rule::of(char_of(APOSTROPHE).then(Pattern::raw(IDENT)), Scope::Label),
    ]
}
