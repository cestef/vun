//! Fields a production restricts by a table: to kinds it lists (`.of`), to kinds ending with one
//! through transparent kinds (`.ending`), or to nodes starting with a token it lists
//! (`.starting`). A kind reaching a listed one through a field of its own is written again with
//! that field restricted, as a hidden rule named after the kind and the table and aliased to the
//! kind, so the tree names it as ever. Operand contexts (`context`) make hidden rules the same way.

use vun_tree_sitter::{Assoc, Rule};

use super::{At, Named, Rules};
use crate::grammar::model::{Kind, Restriction, Table, Term, Token};

impl Rules<'_> {
    /// What a restricted field holds.
    pub(super) fn restricted_to(&self, restriction: &Restriction) -> Rule {
        match restriction {
            Restriction::Of(table) => self.of_table(table, false),
            Restriction::Ending(table) => self.of_table(table, true),
            Restriction::Starting(table) => self.starting(table),
        }
    }

    /// A node of a kind `table` lists, or of a transparent kind ending with one: that kind's
    /// production with its last field holding what this does, as a hidden rule named after both
    /// and aliased to the kind.
    fn of_table(&self, table: &Table<Kind>, ending: bool) -> Rule {
        let name = |k: Kind| self.model.kind_name(k);
        let mut kinds: Vec<Rule> = table.members.iter().map(|&k| Rule::symbol(name(k))).collect();
        let transparent = if ending { self.model.transparent.as_slice() } else { &[] };
        for &kind in transparent {
            let hidden = ["_", name(kind), "_", &table.name].concat();
            if !self.defined(&hidden) {
                self.define(hidden.clone(), Rule::seq(Vec::new()));
                let terms = self.model.production(kind).unwrap_or_default();
                if let Some((Term::Field(field, None), rest)) = terms.split_last() {
                    let mut body: Vec<Rule> = rest.iter().filter_map(|t| self.term(t, At::RANKED)).collect();
                    body.push(self.of_table(table, true).field(self.model.field_name(*field)));
                    let level = self.levels.get(&kind).copied().unwrap_or_default();
                    self.define(hidden.clone(), Rule::seq(body).prec(Assoc::Right, level));
                }
            }
            kinds.push(Rule::symbol(&hidden).alias(name(kind)));
        }
        Rule::choice(kinds)
    }

    /// A node starting with a token `table` names: of a kind whose production starts with such
    /// a token (its first term narrowed to them where it names others too), or of a kind starting
    /// with an operand, that operand starting so. All of them are the hidden rule
    /// `_starting_table`, which the variants refer to.
    fn starting(&self, table: &Table<Token>) -> Rule {
        let all = ["_starting_", &table.name].concat();
        if self.defined(&all) {
            return Rule::symbol(&all);
        }
        self.define(all.clone(), Rule::seq(Vec::new()));
        let mut kinds = Vec::new();
        for (kind, terms) in self.model.produced().filter(|&(k, _)| self.is_operand(k)) {
            let Some((first, rest)) = terms.split_first() else { continue };
            let name = self.model.kind_name(kind);
            let narrowed = match first {
                Term::Tokens(tokens) => {
                    let at: Vec<Token> =
                        tokens.iter().copied().filter(|t| table.members.contains(t)).collect();
                    if at.len() == tokens.len() {
                        kinds.push(Rule::symbol(name));
                        continue;
                    }
                    if at.is_empty() {
                        continue;
                    }
                    Rule::choice(at.into_iter().map(|t| self.token(t, true)).collect())
                }
                Term::Field(field, _) if self.holds_operands(*field) => {
                    Rule::symbol(&all).field(self.model.field_name(*field))
                }
                _ => continue,
            };
            let mut body = vec![narrowed];
            body.extend(rest.iter().filter_map(|t| self.term(t, At::RANKED)));
            let level = self.levels.get(&kind).copied().unwrap_or_default();
            let hidden = ["_", name, "_", &table.name].concat();
            self.define(hidden.clone(), Rule::seq(body).prec(self.assoc(terms), level));
            kinds.push(Rule::symbol(&hidden).alias(name));
        }
        self.define(all.clone(), Rule::choice(kinds));
        Rule::symbol(&all)
    }

    /// The hidden rules made for restricted fields and operand contexts.
    pub(crate) fn wrapped(&self) -> Vec<Named> {
        self.wrapped.borrow().clone()
    }

    pub(super) fn defined(&self, name: &str) -> bool {
        self.wrapped.borrow().iter().any(|(n, _)| n == name)
    }

    /// Records the hidden rule `name`, or replaces it.
    pub(super) fn define(&self, name: String, rule: Rule) {
        let mut wrapped = self.wrapped.borrow_mut();
        match wrapped.iter_mut().find(|(n, _)| *n == name) {
            Some(slot) => slot.1 = rule,
            None => wrapped.push((name, rule)),
        }
    }
}
