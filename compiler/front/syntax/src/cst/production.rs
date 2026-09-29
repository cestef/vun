//! Whether a node spells its kind's production (`NodeKind::spelled_by`): its anchor and `punct`
//! children as tokens and its other children by field, in source order.

use vun_grammar::{Element, NodeKind, TRANSPARENT};

use vun_index::Idx;

use super::{Anchor, Cst, Fields, NodeId};
use crate::tokens::{TokenId, Tokens};

impl Cst {
    /// Whether `n`, whose children fill `fields`, spells its kind's production. What recovery
    /// built is exempt: an `Error`, and a node holding one or anchored at an absent token; and so
    /// is a node holding such a node, whose extent recovery decided, taking what `n` would spell
    /// after it: `#[x;` holds the `;` ending its entry.
    pub(super) fn spells(&self, n: NodeId, fields: &Fields, tokens: &Tokens, source: &str) -> bool {
        let recovered =
            |c: NodeId| self.kind(c) == NodeKind::Error || matches!(self.anchor(c), Anchor::Missing(_));
        let built = |c: NodeId| recovered(c) || self.children_rev(c).any(recovered);
        if built(n) || self.children_rev(n).any(built) {
            return true;
        }
        let token = |t: TokenId| Element::Token { kind: tokens.kind(t), text: tokens.range(t).of(source) };
        let anchor = self.anchor(n).token();
        let mut assigned = fields.assigned();
        let mut elements = Vec::with_capacity(self.size(n));
        let mut placed = false;
        for c in self.children(n) {
            let at = self.anchor(c).token();
            if !placed && anchor < at {
                elements.push(token(anchor));
                placed = true;
            }
            elements.push(match self.kind(c) {
                NodeKind::Punct => token(at),
                _ => match assigned.next() {
                    Some((child, field)) if child == c => Element::Child {
                        field,
                        kind: self.kind(c),
                        ends: self.counted(c, tokens),
                        first: tokens.kind(self.first_token(c)),
                    },
                    _ => return false,
                },
            });
        }
        if !placed {
            elements.push(token(anchor));
        }
        self.kind(n).spelled_by(&elements)
    }

    /// The kind `n` counts as where a production names kinds by a table: a transparent node's,
    /// written with a token making it so, that of its last child; `Error` for what recovery
    /// built, which fits any.
    fn counted(&self, mut n: NodeId, tokens: &Tokens) -> NodeKind {
        let transparent = |n: NodeId| {
            let at = tokens.kind(self.anchor(n).token());
            TRANSPARENT.iter().any(|row| row.kind == self.kind(n) && row.tokens.contains(&at))
        };
        while transparent(n) {
            match self.children_rev(n).find(|&c| self.kind(c) != NodeKind::Punct) {
                Some(last) => n = last,
                None => break,
            }
        }
        if matches!(self.anchor(n), Anchor::Missing(_)) { NodeKind::Error } else { self.kind(n) }
    }

    /// The first token of the subtree of `n`, its tokens being contiguous.
    fn first_token(&self, n: NodeId) -> TokenId {
        let subtree = n.index() + 1 - self.size(n)..=n.index();
        subtree.map(|i| self.anchor(NodeId::new(i)).token()).min().unwrap_or_else(|| self.anchor(n).token())
    }
}
