//! Fields a production restricts by a table: to kinds it lists or transparent kinds ending with
//! one (`.of`), or to nodes starting with a token it lists (`.starting`). A kind reaching a listed
//! one through a field of its own is written again with that field restricted, as a hidden rule
//! named after the kind and the table and aliased to the kind, so the tree names it as ever.

use vun_tree_sitter::{Assoc, Rule};

use super::{At, Named, Rules};
use crate::grammar::model::{Kind, Restriction, Table, Term, Token};

impl Rules<'_> {
    /// What a restricted field holds.
    pub(super) fn restricted_to(&self, restriction: &Restriction) -> Rule {
        match restriction {
            Restriction::Of(table) => self.of_table(table),
            Restriction::Starting(table) => self.starting(table),
        }
    }

    /// A node of a kind `table` lists, or, as the parser's `standing_for`, of a transparent kind
    /// ending with one where no kind listed continues an operand: before such a form, a
    /// transparent node holds the form (`$T{}` is `$(T{})`).
    fn of_table(&self, table: &Table<Kind>) -> Rule {
        let mut kinds: Vec<Rule> =
            table.members.iter().map(|&k| Rule::symbol(self.model.kind_name(k))).collect();
        if !table.members.iter().any(|&k| self.continues(k)) {
            kinds.extend(self.transparent_holding(&table.name, &|| self.of_table(table)));
        }
        Rule::choice(kinds)
    }

    /// Each transparent kind written as holding in its last field what `held` makes: its
    /// production, its first token narrowed to those making it transparent, as a hidden rule
    /// named after the kind and `name` and aliased to the kind.
    pub(super) fn transparent_holding(&self, name: &str, held: &dyn Fn() -> Rule) -> Vec<Rule> {
        let mut rules = Vec::new();
        for (kind, tokens) in &self.model.transparent {
            let kind_name = self.model.kind_name(*kind);
            let hidden = ["_", kind_name, "_", name].concat();
            if !self.defined(&hidden) {
                self.define(hidden.clone(), Rule::seq(Vec::new()));
                let terms = self.model.production(*kind).unwrap_or_default();
                if let Some((Term::Field(field, None), rest)) = terms.split_last() {
                    let mut body: Vec<Rule> = rest
                        .iter()
                        .enumerate()
                        .filter_map(
                            |(i, t)| if i == 0 { self.narrowed(t, tokens) } else { self.term(t, At::RANKED) },
                        )
                        .collect();
                    body.push(held().field(self.model.field_name(*field)));
                    let level = self.levels.get(kind).copied().unwrap_or_default();
                    self.define(hidden.clone(), Rule::seq(body).prec(Assoc::Right, level));
                }
            }
            rules.push(Rule::symbol(&hidden).alias(kind_name));
        }
        rules
    }

    /// `term` starting only with one of `tokens`: a token term narrowed to them, the alternatives
    /// of a choice that may so, a sequence by its first term; `None` where it may not.
    fn narrowed(&self, term: &Term, tokens: &[Token]) -> Option<Rule> {
        match term {
            Term::Tokens(own) => {
                let at: Vec<Rule> =
                    own.iter().filter(|t| tokens.contains(t)).map(|&t| self.token(t, true)).collect();
                (!at.is_empty()).then(|| Rule::choice(at))
            }
            Term::Choice(terms) => {
                let at: Vec<Rule> = terms.iter().filter_map(|t| self.narrowed(t, tokens)).collect();
                (!at.is_empty()).then(|| Rule::choice(at))
            }
            Term::Sequence(terms) => {
                let (first, rest) = terms.split_first()?;
                let mut body = vec![self.narrowed(first, tokens)?];
                body.extend(rest.iter().filter_map(|t| self.term(t, At::RANKED)));
                Some(Rule::seq(body))
            }
            _ => self.term(term, At::RANKED),
        }
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
                Term::Field(field, _) if self.holds_operands(*field) => {
                    Rule::symbol(&all).field(self.model.field_name(*field))
                }
                _ => {
                    let tokens = Term::first_tokens(terms);
                    let at = tokens.iter().filter(|t| table.members.contains(t)).count();
                    if at == 0 {
                        continue;
                    }
                    if at == tokens.len() {
                        kinds.push(Rule::symbol(name));
                        continue;
                    }
                    let Some(narrowed) = self.narrowed(first, &table.members) else { continue };
                    narrowed
                }
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

    /// The hidden rules made for restricted fields.
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
