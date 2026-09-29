//! Queries of a file's structure: folds, indents and brackets from the kinds whose productions
//! hold a pair of the table `delimiters`, and the outline and text objects from the kinds editors
//! navigate by.

use vun_syntax::literal::Literal;
use vun_tree_sitter::Pattern;

use super::super::rules::{Rules, table};
use super::{Capture, Typed};
use crate::grammar::production::Resolved;

/// The table of delimiters and the fields of its rows naming an opening and a closing token.
const DELIMITERS: &str = "delimiters";
const OPEN: &str = "open";
const CLOSE: &str = "close";

/// What the outline lists: declarations by their name.
const DECLARATIONS: [&str; 3] = ["binding", "typed_binding", "field"];
const DECLARED: (&str, &str) = ("pattern", "name");

/// What text objects select: functions and their bodies, types, parameters, comments.
const FUNCTIONS: [&str; 2] = ["fn", "closure"];
const BODY: (&str, &str) = ("block", "block");
const TYPES: [&str; 2] = ["struct", "enum"];
const PARAMETER: &str = "param";
const COMMENTS: [&str; 2] = ["comment", "doc_comment"];

/// Each kind holding a delimited group, with the spellings of its first pair.
fn delimited<'s>(rules: &Rules<'_, 's>) -> Vec<(&'s str, &'s str, &'s str)> {
    let rows = table(rules.decls, DELIMITERS).ok().and_then(Literal::as_list).unwrap_or_default();
    let spelled = |name: Option<&str>| {
        let v = rules.tokens.kind.variants.iter().find(|v| Some(v.name) == name)?;
        match &v.text {
            Some(std::borrow::Cow::Borrowed(text)) => Some(*text),
            _ => None,
        }
    };
    let pairs: Vec<(&str, &str)> = rows
        .iter()
        .filter_map(|r| Some((spelled(r.get(OPEN)?.as_name())?, spelled(r.get(CLOSE)?.as_name())?)))
        .collect();
    let mut found = Vec::new();
    for (kind, terms) in &rules.produced {
        let mut leaves = Vec::new();
        for t in terms {
            t.leaves(&mut leaves);
        }
        let spelled: Vec<&str> = leaves
            .iter()
            .filter_map(|l| match l {
                Resolved::Tokens(names) if names.len() == 1 => spelled(Some(names[0])),
                _ => None,
            })
            .collect();
        // The group a kind opens first: a bound's braces, not its register's parentheses.
        let first = spelled.iter().find_map(|t| pairs.iter().find(|(o, c)| o == t && spelled.contains(c)));
        if let Some(&(open, close)) = first {
            found.push((*kind, open, close));
        }
    }
    found
}

pub(super) fn folds(rules: &Rules<'_, '_>) -> Vec<Typed> {
    let kinds = delimited(rules).into_iter().map(|(kind, _, _)| Pattern::node(kind));
    vec![Pattern::alternatives(kinds).capture(Capture::Fold)]
}

/// Each delimited kind indents what it holds; its closing token ends the indent.
pub(super) fn indents(rules: &Rules<'_, '_>) -> Vec<Typed> {
    let kinds = delimited(rules).into_iter().map(|(kind, open, close)| {
        Pattern::node(kind)
            .child(None, Pattern::token(open))
            .child(None, Pattern::token(close).capture(Capture::Outdent))
            .capture(Capture::Indent)
    });
    kinds.collect()
}

pub(super) fn brackets(rules: &Rules<'_, '_>) -> Vec<Typed> {
    let mut pairs: Vec<(&str, &str)> =
        delimited(rules).into_iter().map(|(_, open, close)| (open, close)).collect();
    pairs.dedup();
    pairs.sort_unstable();
    pairs.dedup();
    let brackets = pairs.into_iter().map(|(open, close)| {
        Pattern::any()
            .child(None, Pattern::token(open).capture(Capture::Open))
            .child(None, Pattern::token(close).capture(Capture::Close))
    });
    brackets.collect()
}

pub(super) fn outline() -> Vec<Typed> {
    let declared = || Pattern::node(DECLARED.1).capture(Capture::Name);
    let items = DECLARATIONS
        .map(|kind| Pattern::node(kind).child(Some(DECLARED.0), declared()).capture(Capture::Item));
    items.into()
}

pub(super) fn text_objects() -> Vec<Typed> {
    let any_of = |kinds: &[&str]| Pattern::alternatives(kinds.iter().map(|k| Pattern::node(k)));
    let mut patterns = vec![any_of(&FUNCTIONS).capture(Capture::FunctionAround)];
    patterns.push(
        Pattern::node(FUNCTIONS[0])
            .child(Some(BODY.0), Pattern::node(BODY.1).capture(Capture::FunctionInside)),
    );
    patterns.push(any_of(&TYPES).capture(Capture::TypeAround));
    patterns.push(Pattern::node(PARAMETER).capture(Capture::ParameterInside));
    patterns.push(any_of(&COMMENTS).capture(Capture::CommentAround));
    patterns
}
