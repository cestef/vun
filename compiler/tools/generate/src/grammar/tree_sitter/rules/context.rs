//! Operand contexts: fields whose operands, at any depth, read as `operand_contexts` says. A
//! context's operand is a hidden rule of its own, `_operand_in_field`, holding the context's kinds
//! and every other operand kind but those starting with a token one of them starts with, each
//! kind holding operands written again with them read in the context, as a hidden rule aliased to
//! the kind.

use vun_tree_sitter::Rule;

use super::Rules;
use crate::grammar::model::{Field, Kind, Term, Token};

impl Rules<'_> {
    /// The context of the field `field`, if it has one.
    pub(super) fn context_holding(&self, field: Field) -> Option<usize> {
        self.model.operand_contexts.iter().position(|c| c.fields.contains(&field))
    }

    /// The hidden rule of an operand read in the context at `index`.
    pub(super) fn context_operand(&self, index: usize) -> Rule {
        let context = &self.model.operand_contexts[index];
        let suffix = ["_in_", self.model.field_name(context.fields[0])].concat();
        let name = ["_operand", &suffix].concat();
        if self.defined(&name) {
            return Rule::symbol(&name);
        }
        self.define(name.clone(), Rule::seq(Vec::new()));
        let starts: Vec<Token> = context.kinds.iter().flat_map(|&k| self.first_tokens(k)).collect();
        let mut kinds: Vec<Rule> =
            context.kinds.iter().map(|&k| Rule::symbol(self.model.kind_name(k))).collect();
        for (kind, terms) in self.model.produced().filter(|&(k, _)| self.is_operand(k)) {
            let kind_name = self.model.kind_name(kind);
            if self.first_tokens(kind).iter().any(|t| starts.contains(t)) {
                continue;
            }
            if !self.model.slots(kind).iter().any(|s| self.holds_operands(s.field)) {
                kinds.push(Rule::symbol(kind_name));
                continue;
            }
            let hidden = ["_", kind_name, &suffix].concat();
            self.define(hidden.clone(), self.rule(kind, terms, Some(index)));
            kinds.push(Rule::symbol(&hidden).alias(kind_name));
        }
        self.define(name.clone(), Rule::choice(kinds));
        Rule::symbol(&name)
    }

    /// The tokens a production of `kind` may start with.
    fn first_tokens(&self, kind: Kind) -> Vec<Token> {
        self.model.production(kind).map(Term::first_tokens).unwrap_or_default()
    }
}
