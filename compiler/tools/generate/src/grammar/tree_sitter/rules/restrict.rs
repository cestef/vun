//! Fields a production restricts by a table: to kinds it lists (`.of`), to kinds ending with one
//! through transparent kinds (`.ending`), or to nodes starting with a token it lists
//! (`.starting`). A kind reaching a listed one through a field of its own is written again with
//! that field restricted, as a hidden rule named after the kind and the table and aliased to the
//! kind, so the tree names it as ever.

use vun_tree_sitter::{Assoc, Rule};

use super::{Named, Rules, TRANSPARENT};
use crate::grammar::production::{Resolved, Restriction, Term};

impl<'s> Rules<'_, 's> {
    /// What a restricted field holds.
    pub(super) fn restricted_to(&self, restriction: Restriction<'s>) -> Option<Rule> {
        match restriction {
            Restriction::Of(table) => self.of_table(table, false),
            Restriction::Ending(table) => self.of_table(table, true),
            Restriction::Starting(table) => self.starting(table),
        }
    }

    /// A node of a kind `table` lists, or of a transparent kind ending with one: that kind's
    /// production with its last field holding what this does, as a hidden rule named after both
    /// and aliased to the kind.
    fn of_table(&self, table: &str, ending: bool) -> Option<Rule> {
        let mut kinds: Vec<Rule> = self.kinds(table).ok()?.iter().map(|k| Rule::symbol(k)).collect();
        let transparent = if ending { self.kinds(TRANSPARENT).ok()? } else { Vec::new() };
        for kind in transparent {
            let name = ["_", kind, "_", table].concat();
            let (_, terms) = self.produced.iter().find(|(k, _)| *k == kind)?;
            if !self.wrapped.borrow().iter().any(|(n, _)| *n == name) {
                self.wrapped.borrow_mut().push((name.clone(), Rule::seq(Vec::new())));
                let (last, rest) = terms.split_last()?;
                let Term::Field(field, None) = last else { return None };
                let mut body: Vec<Rule> = rest.iter().filter_map(|t| self.term(t, true)).collect();
                body.push(self.of_table(table, true)?.field(field));
                let rule =
                    Rule::seq(body).prec(Assoc::Right, self.levels.get(kind).copied().unwrap_or_default());
                let mut wrapped = self.wrapped.borrow_mut();
                if let Some(slot) = wrapped.iter_mut().find(|(n, _)| *n == name) {
                    slot.1 = rule;
                }
            }
            kinds.push(Rule::symbol(&name).alias(kind));
        }
        Some(Rule::choice(kinds))
    }

    /// The hidden rules of transparent kinds made by [`Self::of_table`].
    pub(crate) fn wrapped(&self) -> Vec<Named> {
        self.wrapped.borrow().clone()
    }

    /// A node starting with a token `table` names: of a kind whose production starts with such
    /// a token (its first term narrowed to them where it names others too), or of a kind starting
    /// with an operand, that operand starting so. All of them are the hidden rule
    /// `_starting_table`, which the variants refer to.
    fn starting(&self, table: &str) -> Option<Rule> {
        let all = ["_starting_", table].concat();
        if self.wrapped.borrow().iter().any(|(n, _)| *n == all) {
            return Some(Rule::symbol(&all));
        }
        self.define(all.clone(), Rule::seq(Vec::new()));
        let allowed = self.tokens.names(table)?;
        let mut kinds = Vec::new();
        for (kind, terms) in self.produced.iter().filter(|(k, _)| self.is_operand(k)) {
            let Some((first, rest)) = terms.split_first() else { continue };
            let narrowed = match first {
                Term::Leaf(Resolved::Tokens(names)) => {
                    let at: Vec<&str> = names.iter().copied().filter(|n| allowed.contains(n)).collect();
                    if at.len() == names.len() {
                        kinds.push(Rule::symbol(kind));
                        continue;
                    }
                    if at.is_empty() {
                        continue;
                    }
                    Rule::choice(at.iter().map(|n| self.token(n, true)).collect())
                }
                Term::Field(field, _) if self.holds_operands(field) => Rule::symbol(&all).field(field),
                _ => continue,
            };
            let mut body = vec![narrowed];
            body.extend(rest.iter().filter_map(|t| self.term(t, true)));
            let level = self.levels.get(kind).copied().unwrap_or_default();
            let name = ["_", kind, "_", table].concat();
            self.define(name.clone(), Rule::seq(body).prec(self.assoc(terms), level));
            kinds.push(Rule::symbol(&name).alias(kind));
        }
        self.define(all.clone(), Rule::choice(kinds));
        Some(Rule::symbol(&all))
    }

    /// Records the hidden rule `name`, once.
    fn define(&self, name: String, rule: Rule) {
        let mut wrapped = self.wrapped.borrow_mut();
        match wrapped.iter_mut().find(|(n, _)| *n == name) {
            Some(slot) => slot.1 = rule,
            None => wrapped.push((name, rule)),
        }
    }
}
