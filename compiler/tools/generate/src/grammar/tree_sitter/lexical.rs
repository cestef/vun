//! The tokens the lexer reads by hand, as tree-sitter rules composed from the shared pieces of
//! `grammar::lexical`: each unspelled token kind of `grammar.vun` a production names is a hidden
//! rule `_kind`, and each extra, a comment, a rule of its own; doc comments outrank comments where
//! both match.

use vun_syntax::lexer::{COMMENT, DOC_COMMENT, ESCAPE, MODULE_DOC_COMMENT, RADIX_PREFIXES, RAW_STRING_LINE};

use crate::grammar::lexical::{
    APOSTROPHE, DECIMAL, DOT, EXPONENT, IDENT, LINE_END, QUOTE, RADIX_LEAD, SIGNS, char_of, digits, escape,
};
use crate::grammar::pattern::Pattern;
use vun_tree_sitter::{Assoc, Rule};

/// A token kind's name in `grammar.vun` and how its rule is made.
type Maker = (&'static str, fn() -> Rule);

const TOKENS: [Maker; 12] = [
    ("comment", comment),
    ("ident", ident),
    ("raw_ident", raw_ident),
    ("int", int),
    ("float", float),
    ("char", character),
    ("string", string),
    ("raw_string", raw_string),
    ("raw_string_line", raw_string_line),
    ("label", label),
    ("doc_comment", doc_comment),
    ("module_doc_comment", module_doc_comment),
];

/// Lexical precedences: a doc comment over a plain one matching as long.
const DOC_OVER_COMMENT: i32 = 1;
const MODULE_DOC_OVER_DOC: i32 = 2;

/// The rule of the unspelled token kind `name`, if this module has one.
pub(super) fn rule(name: &str) -> Option<Rule> {
    TOKENS.iter().find(|(n, _)| *n == name).map(|(_, rule)| rule())
}

/// `//` and the rest of its line.
fn comment() -> Rule {
    Rule::seq(vec![text(COMMENT), rest_of_line()]).token()
}

fn pattern(p: &Pattern) -> Rule {
    Rule::Pattern(p.to_string())
}

fn text(t: &str) -> Rule {
    Rule::Text(t.to_owned())
}

fn rest_of_line() -> Rule {
    pattern(&Pattern::except([LINE_END]).many())
}

fn ident() -> Rule {
    pattern(&Pattern::raw(IDENT))
}

fn raw_ident() -> Rule {
    Rule::seq(vec![text(&ESCAPE.to_string()), ident()]).token()
}

/// A run of decimal digits, separators allowed after the first.
fn run() -> Pattern {
    Pattern::raw("[0-9]").then(digits(DECIMAL).many())
}

fn int() -> Rule {
    let prefixed = RADIX_PREFIXES.iter().map(|&(prefix, radix)| {
        char_of(RADIX_LEAD).then(char_of(prefix)).then(digits(radix)).then(digits(radix).many())
    });
    pattern(&Pattern::any(prefixed.chain([run()])))
}

fn float() -> Rule {
    let fraction = char_of(DOT).then(run());
    let exponent = Pattern::class(EXPONENT).then(Pattern::class(SIGNS).optional()).then(run());
    let with_fraction = run().then(fraction).then(exponent.clone().optional());
    pattern(&Pattern::any([with_fraction, run().then(exponent)]))
}

fn character() -> Rule {
    let plain = Pattern::except([APOSTROPHE, ESCAPE, LINE_END]);
    pattern(&char_of(APOSTROPHE).then(Pattern::any([escape(), plain])).then(char_of(APOSTROPHE)))
}

fn string() -> Rule {
    let plain = Pattern::except([QUOTE, ESCAPE, LINE_END]);
    let any_escape = char_of(ESCAPE).then(Pattern::except([LINE_END]));
    pattern(&char_of(QUOTE).then(Pattern::any([plain, any_escape]).many()).then(char_of(QUOTE)))
}

fn raw_string() -> Rule {
    let quote = char_of(QUOTE);
    pattern(&char_of(ESCAPE).then(quote.clone()).then(Pattern::except([QUOTE, LINE_END]).many()).then(quote))
}

fn raw_string_line() -> Rule {
    Rule::seq(vec![text(RAW_STRING_LINE), rest_of_line()]).token()
}

fn label() -> Rule {
    pattern(&char_of(APOSTROPHE).then(Pattern::raw(IDENT)))
}

/// `///` and its line, unless a fourth `/` makes it a plain comment.
fn doc_comment() -> Rule {
    let mark = DOC_COMMENT.strip_prefix(COMMENT).expect("a doc comment starts as a comment");
    let first = mark.chars().next().expect("a doc comment's mark is a character");
    let rest =
        pattern(&Pattern::except([first, LINE_END]).then(Pattern::except([LINE_END]).many()).optional());
    Rule::seq(vec![text(DOC_COMMENT), rest]).prec(Assoc::None, DOC_OVER_COMMENT).token()
}

fn module_doc_comment() -> Rule {
    Rule::seq(vec![text(MODULE_DOC_COMMENT), rest_of_line()]).prec(Assoc::None, MODULE_DOC_OVER_DOC).token()
}
