//! Whether a node spells its kind's production (`NodeKind::spelled_by`): its anchor and `punct`
//! children as tokens and its other children by field, in source order. The verifier holds every
//! node the parser builds to it; the parser chooses by it among kinds spelling the same source.

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
        let recovered = |c: NodeId| {
            matches!(self.kind(c), NodeKind::Error | NodeKind::Skipped)
                || matches!(self.anchor(c), Anchor::Missing(_))
        };
        let built = |c: NodeId| recovered(c) || self.children_rev(c).any(recovered);
        if built(n) || self.children_rev(n).any(built) {
            return true;
        }
        self.elements(n, fields, tokens, source).is_some_and(|elements| self.kind(n).spelled_by(&elements))
    }

    /// The anchor and children of `n`, whose children fill `fields`, as its production reads them:
    /// `None` where they do not line up.
    fn elements<'s>(
        &self,
        n: NodeId,
        fields: &Fields,
        tokens: &Tokens,
        source: &'s str,
    ) -> Option<Vec<Element<'s>>> {
        let token = |t: TokenId| Element::Token { kind: tokens.kind(t), text: tokens.range(t).of(source) };
        let anchor = self.anchor(n).token();
        let mut assigned = fields.assigned();
        let mut elements = Vec::with_capacity(self.size(n));
        let mut placed = false;
        // A skipped token is read by no production.
        for c in self.children(n).filter(|&c| self.kind(c) != NodeKind::Skipped) {
            let at = self.anchor(c).token();
            if !placed && anchor < at {
                elements.push(token(anchor));
                placed = true;
            }
            elements.push(match (self.kind(c), self.anchor(c)) {
                (NodeKind::Punct, Anchor::Missing(_)) => Element::Absent,
                (NodeKind::Punct, _) => token(at),
                _ => match assigned.next() {
                    Some((child, field)) if child == c => Element::Child {
                        field,
                        kind: self.kind(c),
                        ends: self.counted(c, tokens),
                        first: tokens.kind(self.first_token(c)),
                    },
                    _ => return None,
                },
            });
        }
        if !placed {
            elements.push(token(anchor));
        }
        Some(elements)
    }

    /// The kind `n` counts as where a production names kinds by a table: that of the node it
    /// ends with (`Cst::ends_with`); `Error` for what recovery built, which fits any.
    fn counted(&self, n: NodeId, tokens: &Tokens) -> NodeKind {
        let n = self.ends_with(n, tokens);
        if matches!(self.anchor(n), Anchor::Missing(_)) { NodeKind::Error } else { self.kind(n) }
    }

    /// The node `n` ends with: itself, or for a transparent node written with a token making it
    /// so, the node its last child ends with.
    fn ends_with(&self, mut n: NodeId, tokens: &Tokens) -> NodeId {
        let transparent = |n: NodeId| {
            let at = tokens.kind(self.anchor(n).token());
            TRANSPARENT.iter().any(|row| row.kind == self.kind(n) && row.tokens.contains(&at))
        };
        while transparent(n) {
            match self.children_rev(n).find(|&c| !self.kind(c).is_token()) {
                Some(last) => n = last,
                None => break,
            }
        }
        n
    }

    /// The first token of the subtree of `n`, its tokens being contiguous.
    fn first_token(&self, n: NodeId) -> TokenId {
        let subtree = n.index() + 1 - self.size(n)..=n.index();
        subtree.map(|i| self.anchor(NodeId::new(i)).token()).min().unwrap_or_else(|| self.anchor(n).token())
    }
}
