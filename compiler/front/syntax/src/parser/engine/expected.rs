//! What a reading expects next, as recovery reports it: tokens, or the child a position takes,
//! described by what it may be.

use vun_diag::{Diagnostic, Msg, errors};
use vun_grammar::{NodeKind, Read, TokenSet};

use super::frame::Reading;
use super::spec::Spec;
use crate::parser::Parser;

/// What a reading expects next, the more general the later.
#[derive(Clone, Copy, Debug)]
pub(super) enum Expected {
    Tokens(TokenSet),
    /// A part starting with an operand: a binding, a field.
    Declaration,
    Name,
    Expression,
}

impl Expected {
    /// What a position taking `spec` expects, the kinds it names first: a name, a part continuing
    /// an operand, any operand, or the tokens the kinds it names start with.
    fn child(spec: &Spec) -> Self {
        if spec.named.contains(NodeKind::Name) {
            Self::Name
        } else if spec.named.iter().any(NodeKind::continues) {
            Self::Declaration
        } else if spec.takes_operands() {
            Self::Expression
        } else {
            Self::Tokens(spec.named.iter().fold(TokenSet::EMPTY, |set, k| set.union(k.first())))
        }
    }

    /// What either expects: the more general, or both tokens.
    fn or(self, other: Self) -> Self {
        match (self, other) {
            (Self::Tokens(a), Self::Tokens(b)) => Self::Tokens(a.union(b)),
            (a, b) => {
                if a.rank() >= b.rank() {
                    a
                } else {
                    b
                }
            }
        }
    }

    fn rank(self) -> u8 {
        match self {
            Self::Tokens(_) => 0,
            Self::Declaration => 1,
            Self::Name => 2,
            Self::Expression => 3,
        }
    }
}

impl Parser<'_, '_, '_> {
    /// What the next positions of `r` expect.
    pub(super) fn expected(&self, r: &Reading, outer: Option<&Spec>) -> Expected {
        let mut expected = Expected::Tokens(TokenSet::EMPTY);
        for &c in &r.candidates {
            for p in c.next().iter() {
                let here = match &c.automaton.reads[p] {
                    Read::Token(set) => Expected::Tokens(*set),
                    Read::Word(_) => continue,
                    Read::Child(_) => {
                        self.taking(r, c, p, outer).map_or(Expected::Expression, |s| Expected::child(&s))
                    }
                };
                expected = expected.or(here);
            }
        }
        expected
    }

    /// Reports what was expected where the current token stands.
    pub(super) fn report_expected_what(&mut self, expected: Expected) {
        let (id, what) = match expected {
            Expected::Tokens(tokens) => return self.report_expected(&tokens.iter().collect::<Vec<_>>()),
            Expected::Declaration => (errors::SYNTAX_EXPECTED_DECLARATION, "expected a declaration, found "),
            Expected::Name => (errors::SYNTAX_EXPECTED_NAME, "expected a name, found "),
            Expected::Expression => (errors::SYNTAX_EXPECTED_EXPRESSION, "expected an expression, found "),
        };
        let mut d = Diagnostic::error(id, self.found(Msg::new().text(what)));
        if let Some(keyword) = self.nearby_keyword() {
            d = d.fix(self.escape(keyword));
        }
        self.report(self.gap(), d);
    }
}
