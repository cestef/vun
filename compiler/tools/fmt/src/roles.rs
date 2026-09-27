//! What the CST says about each token for layout: the kind of node it anchors, and whether it
//! starts an entry of a list (an item, a statement, a member, an argument, an arm), which a line
//! starting with it does not continue.

use vun_grammar::{Count, NodeField, NodeKind};
use vun_index::IndexVec;
use vun_syntax::cst::{Cst, NodeId};
use vun_syntax::tokens::{TokenId, Tokens};

pub(crate) struct Roles {
    anchors: IndexVec<TokenId, Option<NodeKind>>,
    entries: IndexVec<TokenId, bool>,
    /// Infix operators without a right operand: `a..`.
    open: IndexVec<TokenId, bool>,
}

impl Roles {
    pub(crate) fn of(tokens: &Tokens, cst: &Cst) -> Self {
        let mut anchors = IndexVec::filled(None, tokens.len());
        let mut first: IndexVec<NodeId, TokenId> = IndexVec::with_capacity(cst.len());
        for n in cst.ids() {
            let token = cst.anchor(n).token();
            anchors[token] = Some(cst.kind(n));
            let children = cst.children_rev(n).map(|c| first[c]);
            first.push(children.chain([token]).min().unwrap_or(token));
        }
        let (entries, open) = (IndexVec::filled(false, tokens.len()), IndexVec::filled(false, tokens.len()));
        let mut roles = Self { anchors, entries, open };
        for n in cst.ids() {
            let Some(fields) = cst.fields(n) else { continue };
            if cst.kind(n) == NodeKind::Infix && fields.one(NodeField::Right).is_none() {
                roles.open[cst.anchor(n).token()] = true;
            }
            for slot in cst.kind(n).slots().iter().filter(|s| s.count == Count::Many) {
                for &entry in fields.get(slot.field) {
                    roles.entry(cst, &first, entry);
                }
            }
        }
        roles
    }

    /// Marks `entry` as starting an entry, and what its docs and attributes mark in turn.
    fn entry(&mut self, cst: &Cst, first: &IndexVec<NodeId, TokenId>, entry: NodeId) {
        self.entries[first[entry]] = true;
        if matches!(cst.kind(entry), NodeKind::Doc | NodeKind::Attr)
            && let Some(target) = cst.fields(entry).and_then(|f| f.one(NodeField::Target))
        {
            self.entry(cst, first, target);
        }
    }

    /// The kind of the node `t` anchors.
    pub(crate) fn anchor(&self, t: TokenId) -> Option<NodeKind> {
        self.anchors[t]
    }

    /// Whether `t` is an infix operator without a right operand.
    pub(crate) fn is_open(&self, t: TokenId) -> bool {
        self.open[t]
    }

    /// Whether `t` starts an entry of a list.
    pub(crate) fn starts_entry(&self, t: TokenId) -> bool {
        self.entries[t]
    }
}
