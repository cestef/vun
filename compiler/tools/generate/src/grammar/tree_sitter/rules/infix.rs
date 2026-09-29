//! The infix form: one alternative per infix level, at the level's precedence, the production
//! spelled out so that a level's operators stand alone in it.

use vun_tree_sitter::{Assoc, Rule};

use super::{At, Rules};
use crate::grammar::model::{Term, Token};

impl Rules<'_> {
    /// The infix form, one alternative per infix level, each at its precedence, its operands read
    /// in `context` if one is given.
    pub(super) fn infix(&self, terms: &[Term], context: Option<usize>) -> Rule {
        let sequences = Term::spelled_out(terms);
        let mut alternatives = Vec::new();
        for (i, operators) in self.model.infix.iter().enumerate() {
            for sequence in &sequences {
                let body: Option<Vec<Rule>> =
                    sequence.iter().map(|t| self.restricted(t, operators, context)).collect();
                let Some(body) = body else { continue };
                // A form without its right operand takes one after it where one follows: `0..(n)`.
                // A chain of a level without grouping is an error the parser reports; parsed, it
                // groups left.
                let closed =
                    matches!(sequence.last(), Some(Term::Field(field, None)) if self.holds_operands(*field));
                let assoc = if closed { Assoc::Left } else { Assoc::Right };
                let level = self.infix_base + i32::try_from(i).unwrap_or(i32::MAX);
                alternatives.push(Rule::seq(body).prec(assoc, level));
            }
        }
        Rule::choice(alternatives)
    }

    /// A term of the infix form at one level: its operator tokens those of `operators` alone;
    /// `None` where it names only operators of other levels, and so cannot stand at this one.
    fn restricted(&self, term: &Term, operators: &[Token], context: Option<usize>) -> Option<Rule> {
        match term {
            Term::Tokens(tokens) => {
                let at: Vec<Rule> =
                    tokens.iter().filter(|t| operators.contains(t)).map(|&t| self.token(t, true)).collect();
                (!at.is_empty()).then(|| Rule::choice(at))
            }
            _ => self.term(term, At { context, ..At::RANKED }),
        }
    }
}
