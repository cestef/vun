//! Highlights: tokens by the tables they are in (keywords, operators, delimiters, separators),
//! leaves by their kind, and names by where they stand, general patterns first.

use vun_tree_sitter::Pattern;

use super::super::lexical::COMMENT_KIND;
use super::super::rules::{Rules, missing};
use super::{Capture, Typed};
use crate::generate::Read;
use crate::grammar::lexical::TYPE_NAME;
use crate::grammar::production::{Resolved, Term};

/// The tables and lists of tokens the highlights read, with what they capture, first first.
const TOKENS: [(&str, Capture); 7] = [
    ("keywords", Capture::Keyword),
    ("assign", Capture::Operator),
    ("infix", Capture::Operator),
    ("prefix", Capture::Operator),
    ("postfix", Capture::Operator),
    ("delimiters", Capture::Bracket),
    ("separators", Capture::Delimiter),
];

/// Node kinds captured whole, and the tokens named after their kind.
const NODES: [(&str, Capture); 15] = [
    (COMMENT_KIND, Capture::Comment),
    ("doc_comment", Capture::Documentation),
    ("module_doc", Capture::Documentation),
    ("int", Capture::Number),
    ("float", Capture::Number),
    ("string", Capture::String),
    ("raw_string_line", Capture::String),
    ("char", Capture::Character),
    ("label", Capture::Label),
    ("hole", Capture::Builtin),
    ("rest", Capture::Operator),
    ("mut", Capture::Keyword),
    ("qualifier", Capture::Keyword),
    ("mode", Capture::Keyword),
    ("flow", Capture::Keyword),
];

/// The leaf names are, and where a name stands that says what it names: a kind, a field of it
/// holding the name (through a field of what it holds, if a second is given), and the capture.
const NAME: &str = "name";
const PLACES: [(&str, &str, Option<&str>, Capture); 5] = [
    ("param", "pattern", None, Capture::Parameter),
    ("member", "property", None, Capture::Property),
    ("dot_name", "property", None, Capture::Variant),
    ("call", "callee", None, Capture::Call),
    ("call", "callee", Some("property"), Capture::Method),
];

/// Bindings whose value says what their name names: the binding kinds, their name and value
/// fields, and the kinds of value with what they make of the name.
const BINDINGS: [&str; 2] = ["binding", "typed_binding"];
const BOUND: (&str, &str) = ("pattern", "value");
const VALUES: [(&[&str], Capture); 2] =
    [(&["fn", "closure"], Capture::Function), (&["struct", "enum"], Capture::Type)];

pub(super) fn patterns(rules: &Rules<'_, '_>) -> Read<Vec<Typed>> {
    let name = || Pattern::node(NAME);
    let mut patterns = vec![name().capture(Capture::Variable)];
    let anchored = ["^", TYPE_NAME, "$"].concat();
    patterns.push(name().capture(Capture::Type).matching(Capture::Type, &anchored));
    let words = words(rules);
    let tokens = |spellings: Vec<&str>| Pattern::alternatives(spellings.into_iter().map(Pattern::token));
    patterns.push(tokens(words.iter().map(String::as_str).collect()).capture(Capture::Keyword));
    // Later tables take a token first, so a keyword operator (`and`) is an operator.
    for (table, capture) in TOKENS.iter().rev() {
        let spellings = rules.tokens.spellings(table).ok_or_else(|| missing(table))?;
        patterns.push(tokens(spellings).capture(*capture));
    }
    for (kind, capture) in NODES {
        patterns.push(Pattern::node(kind).capture(capture));
    }
    for (kind, field, inner, capture) in PLACES {
        let named = match inner {
            Some(inner) => Pattern::any().child(Some(inner), name().capture(capture)),
            None => name().capture(capture),
        };
        patterns.push(Pattern::node(kind).child(Some(field), named));
    }
    for binding in BINDINGS {
        for (values, capture) in VALUES {
            let value = Pattern::alternatives(values.iter().map(|v| Pattern::node(v)));
            let pattern = Pattern::node(binding)
                .child(Some(BOUND.0), name().capture(capture))
                .child(Some(BOUND.1), value);
            patterns.push(pattern);
        }
    }
    Ok(patterns)
}

/// The words branches spell, keywords only where they stand: `in`, `asm`.
fn words(rules: &Rules<'_, '_>) -> Vec<String> {
    let mut words = Vec::new();
    for (_, terms) in &rules.produced {
        let mut leaves = Vec::new();
        for t in terms {
            t.leaves(&mut leaves);
        }
        if leaves.len() > 1 || !matches!(terms.as_slice(), [Term::Leaf(_)]) {
            for leaf in leaves {
                let Resolved::Words(ws) = leaf else { continue };
                for w in ws {
                    if !words.iter().any(|known| known == w) {
                        words.push(w.to_string());
                    }
                }
            }
        }
    }
    words
}
