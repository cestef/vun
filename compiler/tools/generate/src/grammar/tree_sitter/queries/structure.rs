//! Queries of a file's structure: folds, indents and brackets from the kinds whose productions
//! hold a pair of delimiters, and the outline and text objects as `editors.vun` says.

use vun_tree_sitter::Pattern;

use super::super::rules::Rules;
use super::{Capture, Typed, captured};
use crate::grammar::model::{Kind, Term, Token};

/// Each kind holding a delimited group, with its first pair.
fn delimited(rules: &Rules<'_>) -> Vec<(Kind, Token, Token)> {
    let model = rules.model;
    let mut found = Vec::new();
    for (kind, terms) in model.produced() {
        let spelled: Vec<Token> = Term::all_leaves(terms)
            .into_iter()
            .filter_map(|l| match l {
                Term::Tokens(tokens) if tokens.len() == 1 => Some(tokens[0]),
                _ => None,
            })
            .collect();
        // The group a kind opens first: a bound's braces, not its register's parentheses.
        let first =
            spelled.iter().find_map(|t| model.delimiters.iter().find(|(o, c)| o == t && spelled.contains(c)));
        if let Some(&(open, close)) = first {
            found.push((kind, open, close));
        }
    }
    found
}

pub(super) fn folds(rules: &Rules<'_>) -> Vec<Typed> {
    let kinds = delimited(rules).into_iter().map(|(kind, _, _)| Pattern::node(rules.model.kind_name(kind)));
    vec![Pattern::alternatives(kinds).capture(Capture::Fold)]
}

/// Each delimited kind indents what it holds; its closing token ends the indent.
pub(super) fn indents(rules: &Rules<'_>) -> Vec<Typed> {
    let model = rules.model;
    let text = |t: Token| model.text(t).unwrap_or_default();
    let kinds = delimited(rules).into_iter().map(|(kind, open, close)| {
        Pattern::node(model.kind_name(kind))
            .child(None, Pattern::token(text(open)))
            .child(None, Pattern::token(text(close)).capture(Capture::Outdent))
            .capture(Capture::Indent)
    });
    kinds.collect()
}

pub(super) fn brackets(rules: &Rules<'_>) -> Vec<Typed> {
    let model = rules.model;
    let mut pairs: Vec<(&str, &str)> = delimited(rules)
        .into_iter()
        .filter_map(|(_, open, close)| Some((model.text(open)?, model.text(close)?)))
        .collect();
    pairs.sort_unstable();
    pairs.dedup();
    let brackets = pairs.into_iter().map(|(open, close)| {
        Pattern::any()
            .child(None, Pattern::token(open).capture(Capture::Open))
            .child(None, Pattern::token(close).capture(Capture::Close))
    });
    brackets.collect()
}

pub(super) fn outline(rules: &Rules<'_>) -> Vec<Typed> {
    let (model, outline) = (rules.model, &rules.editors.outline);
    let items = outline.kinds.iter().map(|&kind| {
        let declared = Pattern::node(model.kind_name(outline.node)).capture(Capture::Name);
        Pattern::node(model.kind_name(kind))
            .child(Some(model.field_name(outline.field)), declared)
            .capture(Capture::Item)
    });
    items.collect()
}

pub(super) fn text_objects(rules: &Rules<'_>) -> Vec<Typed> {
    captured(rules, &rules.editors.text_objects)
}
