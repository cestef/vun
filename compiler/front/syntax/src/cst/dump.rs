//! The indented text form of a tree, for `vun dump cst` and snapshots.

use std::fmt;

use vun_grammar::NodeField;
use vun_outline::{Outline, Outlined};
use vun_span::ByteRange;

use super::{Anchor, Cst, NodeId};
use crate::tokens::Tokens;

/// See [`Cst::dump`].
pub struct Dump<'a> {
    pub(super) cst: &'a Cst,
    pub(super) tokens: &'a Tokens,
    pub(super) source: &'a str,
}

struct Printer<'d, 'a> {
    dump: &'d Dump<'a>,
    out: Outline,
}

impl Outlined for Printer<'_, '_> {
    fn outline(&mut self) -> &mut Outline {
        &mut self.out
    }
}

impl Printer<'_, '_> {
    /// A node, prefixed by the field it fills in its parent, then its children.
    fn visit(&mut self, n: NodeId, field: Option<NodeField>) {
        let Dump { cst, tokens, source } = *self.dump;
        let field = field.map(|f| format!("{}: ", f.name())).unwrap_or_default();
        let at = match cst.anchor(n) {
            Anchor::Token(t) => {
                let range = tokens.range(t);
                let ByteRange { lo, hi } = range;
                format!("{:?} {lo}..{hi} {:?}", tokens.kind(t), range.of(source))
            }
            Anchor::Missing(t) => format!("missing at {}", tokens.range(t).lo),
        };
        self.node(format_args!("{field}{:?} {at}", cst.kind(n)), |p| {
            let fields = cst.fields(n).unwrap_or_default();
            let mut assigned = fields.assigned().peekable();
            for c in cst.children(n) {
                // Children and assignments are both in source order; punctuation has no field.
                let field = assigned.next_if(|&(node, _)| node == c).map(|(_, field)| field);
                p.visit(c, field);
            }
        });
    }
}

impl fmt::Display for Dump<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.cst.is_empty() {
            return Ok(());
        }
        let mut printer = Printer { dump: self, out: Outline::new() };
        printer.visit(self.cst.root(), None);
        f.write_str(&printer.out.finish())
    }
}
