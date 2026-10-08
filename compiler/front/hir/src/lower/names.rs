//! Names in name positions: a plain name or label with its escape removed, `$(e)`, a tuple
//! position, or `_`.

use vun_diag::{Diagnostic, Msg, errors};
use vun_grammar::{NodeField, NodeKind, TokenKind};
use vun_intern::Symbol;
use vun_syntax::cst::NodeId;

use super::{ESCAPE, LABEL, Lower};
use crate::tree::Name;

impl Lower<'_, '_> {
    /// The symbol of a name leaf, without the escape of `\name`.
    pub(super) fn symbol(&mut self, n: NodeId) -> Symbol {
        let text = self.text(n);
        Symbol::intern(text.strip_prefix(ESCAPE).unwrap_or(text))
    }

    pub(super) fn label(&mut self, n: NodeId) -> Symbol {
        let text = self.text(n);
        Symbol::intern(text.strip_prefix(LABEL).unwrap_or(text))
    }

    /// A name in a name position: plain, `$(e)`, a tuple index, or `_`.
    pub(super) fn name(&mut self, n: NodeId) -> Name {
        match self.kind(n) {
            NodeKind::Name => Name::Plain(self.symbol(n)),
            NodeKind::Int => vun_syntax::literal::integer(self.text(n))
                .and_then(|i| u32::try_from(&i).ok())
                .map_or_else(|| self.oversized_index(n), Name::Index),
            NodeKind::Prefix if self.token(n) == Some(TokenKind::Dollar) => {
                // Parentheses lower to their inner expression.
                let f = self.fields(n);
                Name::Computed(self.operand(&f, n, NodeField::Operand))
            }
            _ => Name::Hole,
        }
    }

    /// Reports the tuple position `n`, which no tuple has, and names nothing.
    fn oversized_index(&mut self, n: NodeId) -> Name {
        let msg = Msg::new().text("a tuple position is at most ").num(u32::MAX);
        let origin = self.origins.source(self.source.cst.span(n, self.source.tokens));
        self.diag.error(Diagnostic::error(errors::SYNTAX_TUPLE_INDEX_TOO_LARGE, msg).at(origin, None));
        Name::Hole
    }
}
