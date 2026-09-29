//! The infix form: one alternative per row of the table `infix`, at the row's level, the
//! production spelled out so that a row's operators stand alone in it.

use vun_syntax::literal::Literal;
use vun_tree_sitter::{Assoc, Rule};

use super::{INFIX, Rules, TOKENS, missing, table};
use crate::generate::Read;
use crate::grammar::production::{Resolved, Term};

impl<'s> Rules<'_, 's> {
    /// The infix form, one alternative per row of the infix table, each at its level.
    pub(super) fn infix(&self, terms: &[Term<'s, Resolved<'s>>]) -> Read<Rule> {
        let rows = table(self.decls, INFIX)?.as_list().ok_or_else(|| missing(INFIX))?;
        let sequences = Term::spelled_out(terms);
        let mut alternatives = Vec::new();
        for (i, row) in rows.iter().enumerate() {
            let operators = row.get(TOKENS).and_then(Literal::as_list).ok_or_else(|| missing(TOKENS))?;
            let operators: Vec<&str> = operators.iter().filter_map(Literal::as_name).collect();
            for sequence in &sequences {
                let body: Option<Vec<Rule>> =
                    sequence.iter().map(|t| self.restricted(t, &operators)).collect();
                let Some(body) = body else { continue };
                // A form without its right operand takes one after it where one follows: `0..(n)`.
                // A chain of a level without grouping is an error the parser reports; parsed, it
                // groups left.
                let closed =
                    matches!(sequence.last(), Some(Term::Field(field, None)) if self.holds_operands(field));
                let assoc = if closed { Assoc::Left } else { Assoc::Right };
                let level = self.infix_base + i32::try_from(i).unwrap_or(i32::MAX);
                alternatives.push(Rule::seq(body).prec(assoc, level));
            }
        }
        Ok(Rule::choice(alternatives))
    }

    /// A term of the infix form at one level: its operator tokens those of `operators` alone;
    /// `None` where it names only operators of other levels, and so cannot stand at this one.
    fn restricted(&self, term: &Term<'s, Resolved<'s>>, operators: &[&str]) -> Option<Rule> {
        match term {
            Term::Leaf(Resolved::Tokens(names)) => {
                let at = names.iter().filter(|n| operators.contains(n)).map(|n| self.token(n, true));
                let at: Vec<Rule> = at.collect();
                (!at.is_empty()).then(|| Rule::choice(at))
            }
            _ => self.term(term, true),
        }
    }
}
