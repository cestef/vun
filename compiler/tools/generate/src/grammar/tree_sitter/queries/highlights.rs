//! Highlights: tokens by the tables they are in, nodes by their kind, and names by where they
//! stand, as `editors.vun` says, general patterns first.

use vun_tree_sitter::Pattern;

use super::super::rules::Rules;
use super::{Capture, Typed, captured};
use crate::grammar::lexical::TYPE_NAME;
use crate::grammar::model::Term;

pub(super) fn patterns(rules: &Rules<'_>) -> Vec<Typed> {
    let (model, highlights) = (rules.model, &rules.editors.highlights);
    let name = || Pattern::node(model.kind_name(highlights.name));
    let mut patterns = vec![name().capture(Capture::Variable)];
    let anchored = ["^", TYPE_NAME, "$"].concat();
    patterns.push(name().capture(Capture::Type).matching(Capture::Type, &anchored));
    let tokens = |spellings: Vec<&str>| Pattern::alternatives(spellings.into_iter().map(Pattern::token));
    patterns.push(tokens(words(rules).iter().map(String::as_str).collect()).capture(Capture::Keyword));
    for (listed, capture) in &highlights.tokens {
        patterns.push(tokens(listed.iter().filter_map(|&t| model.text(t)).collect()).capture(*capture));
    }
    patterns.extend(captured(rules, &highlights.captures));
    for binding in &highlights.bindings {
        let value = Pattern::alternatives(binding.values.iter().map(|&k| Pattern::node(model.kind_name(k))));
        for &kind in &binding.kinds {
            let pattern = Pattern::node(model.kind_name(kind))
                .child(Some(model.field_name(binding.name)), name().capture(binding.capture))
                .child(Some(model.field_name(binding.value)), value.clone());
            patterns.push(pattern);
        }
    }
    patterns
}

/// The words branches spell, keywords only where they stand: `in`, `asm`.
fn words(rules: &Rules<'_>) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    for (_, terms) in rules.model.produced() {
        let leaves = Term::all_leaves(terms);
        if leaves.len() > 1 || !matches!(terms, [Term::Tokens(_) | Term::Words(_)]) {
            for leaf in leaves {
                let Term::Words(ws) = leaf else { continue };
                for w in ws {
                    if !words.contains(w) {
                        words.push(w.clone());
                    }
                }
            }
        }
    }
    words
}
