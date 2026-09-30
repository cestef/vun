//! Productions beginning with an operand, which continue one read before them, the tokens the
//! others start with alone, and the kinds by preference.

use crate::{NodeField, NodeKind, TokenKind};

use super::derived::DERIVED;
use super::{Child, KindSet};

impl Child {
    /// Whether this child may be an operand: any, or one of the kinds it names.
    #[must_use]
    pub fn takes_operands(&self) -> bool {
        self.takes_operands_among(NodeKind::operands())
    }

    /// Whether this child may be one of `operands`.
    pub(super) fn takes_operands_among(&self, operands: KindSet) -> bool {
        self.kinds.is_none_or(|kinds| !kinds.intersection(operands).is_empty())
    }

    /// Whether the kinds this child names may be those of nodes transparent ones end with.
    #[must_use]
    pub fn through(&self) -> bool {
        self.kinds.is_some() && !self.exact
    }

    /// Whether a node of `kind` may stand here as it is, besides the kinds `context` makes
    /// operands.
    #[must_use]
    pub fn holds(&self, kind: NodeKind, context: KindSet) -> bool {
        let held = match self.kinds {
            Some(kinds) => kinds.contains(kind),
            None => NodeKind::operands().contains(kind) || self.admitted.contains(kind),
        };
        held || context.contains(kind) || kind == NodeKind::Error
    }
}

impl NodeField {
    /// The parts that may lead to one this field admits, those included (`n := p` leads to
    /// `n := p = v` in a head).
    #[must_use]
    pub fn leading(self) -> KindSet {
        DERIVED.field_leading[self as usize]
    }
}

impl NodeKind {
    /// Whether a node of this kind may continue an operand read before it.
    #[must_use]
    pub fn continues(self) -> bool {
        DERIVED.continuing.contains(self)
    }

    /// The kinds whose nodes may continue an operand read before them.
    #[must_use]
    pub fn continuing() -> KindSet {
        DERIVED.continuing
    }

    /// The kinds with productions, preferred first.
    #[must_use]
    pub fn by_rank() -> &'static [Self] {
        &DERIVED.by_rank
    }

    /// Those continuing an operand, preferred first.
    #[must_use]
    pub fn continuing_by_rank() -> &'static [Self] {
        &DERIVED.continuing_by_rank
    }

    /// Where this kind stands among [`NodeKind::by_rank`]; past them all without a production.
    #[must_use]
    pub fn preference(self) -> usize {
        DERIVED.ranked[self.index()]
    }

    /// The kinds that are transparent where written with some token.
    #[must_use]
    pub fn transparent() -> KindSet {
        DERIVED.transparent
    }
}

impl TokenKind {
    /// The transparent kinds written so with this token.
    #[must_use]
    pub fn transparent(self) -> KindSet {
        DERIVED.transparent_with[self as usize]
    }
}

impl KindSet {
    /// These kinds, and the parts that may lead to one of them, and so on.
    #[must_use]
    pub fn leading(self) -> Self {
        self.iter().fold(self, |set, k| set.union(DERIVED.leads_to[k.index()]))
    }
}
