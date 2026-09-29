//! Whether a node spells its kind's production (`NodeKind::spelled_by`): its anchor and `punct`
//! children as tokens and its other children by field, in source order. The verifier holds every
//! node the parser builds to it; the parser chooses by it among kinds spelling the same source.

use vun_grammar::{Element, NodeKind, TRANSPARENT};

use vun_index::Idx;
use vun_trace::tracing;

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
        self.elements(n, fields, tokens, source).is_some_and(|elements| self.kind(n).spelled_by(&elements))
    }

    /// Whether `n` spells its kind's production with each child in a field admitting it
    /// (`NodeKind::admits`), `context` admitted everywhere; what recovery built fits as it stands.
    pub(crate) fn fits(&self, n: NodeId, context: &[NodeKind], tokens: &Tokens, source: &str) -> bool {
        let kind = self.kind(n);
        let Some(fields) = self.fields(n) else {
            tracing::trace!(?kind, "its children fit no assignment to its slots");
            return false;
        };
        let Some(elements) = self.elements(n, &fields, tokens, source) else {
            tracing::trace!(?kind, "its children do not line up with its fields");
            return false;
        };
        let refused = elements.iter().find(|e| match **e {
            Element::Child { field, kind: child, .. } => !kind.admits(field, child, context),
            Element::Token { .. } | Element::Absent => false,
        });
        if let Some(child) = refused {
            tracing::trace!(?kind, ?child, "a field does not admit its child");
            return false;
        }
        let spelled = kind.spelled_by(&elements);
        if !spelled {
            tracing::trace!(?kind, ?elements, "its children do not spell its production");
        }
        spelled
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
        for c in self.children(n) {
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
