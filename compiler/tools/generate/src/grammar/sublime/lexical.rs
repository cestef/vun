//! Sublime rules for the tokens the grammar has no table of, since the lexer reads them by hand:
//! comments, strings, characters, numbers, labels and escaped names, composed from the shared
//! pieces of `grammar::lexical`.

use vun_syntax::lexer::{
    C_STRING, COMMENT, DOC_COMMENT, ESCAPE, MODULE_DOC_COMMENT, RADIX_PREFIXES, RAW_STRING_LINE,
};

use super::syntax::{Context, Rule, Scope, Then};
use crate::grammar::lexical::{
    APOSTROPHE, DECIMAL, DOT, EXPONENT, IDENT, LINE_END, QUOTE, RADIX_LEAD, SIGNS, char_of, digits, escape,
};
use crate::grammar::pattern::Pattern;

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
        Rule::of(char_of(C_STRING).optional().then(quote.clone()), Scope::StringBegin)
            .then(Then::Push(Context::String)),
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
