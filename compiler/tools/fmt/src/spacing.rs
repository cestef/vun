//! Spaces within a line (spec 13, section 5): one between two tokens, unless a table of the
//! grammar or the role of one of them in the CST says none.

use vun_grammar::{DelimitersRow, NodeKind, SEPARATORS, Spacing, TokenKind};

/// A token as spacing reads it: its kind, and the kind of the node it anchors.
#[derive(Clone, Copy)]
pub(crate) struct Tok {
    pub(crate) kind: TokenKind,
    pub(crate) anchor: Option<NodeKind>,
    /// Whether it is an infix operator without a right operand: `a..`.
    pub(crate) open: bool,
}

/// A delimited group: its row of `delimiters`, and the kind of node its opener anchors.
#[derive(Clone, Copy)]
pub(crate) struct Group {
    pub(crate) row: DelimitersRow,
    pub(crate) anchor: Option<NodeKind>,
}

impl Group {
    /// The spacing inside the group: its row's, except that a value bound to an instruction's
    /// operand is tight, `{in a}`.
    fn inside(self) -> Spacing {
        if self.anchor == Some(NodeKind::Bound) { Spacing::Tight } else { self.row.inside }
    }
}

/// Where two tokens stand towards the groups around them.
pub(crate) struct Between {
    /// The group `next` closes, if it closes one.
    pub(crate) closing: Option<Group>,
    /// The kind of node the opener of the group `prev` closes anchors, if `prev` closes one:
    /// `[4]u8` is an array type.
    pub(crate) closed: Option<NodeKind>,
}

impl Tok {
    /// Whether one space separates this token from `next` on a line.
    pub(crate) fn spaced(self, next: Self, between: &Between) -> bool {
        if let Some(row) = self.kind.delimiter() {
            // An empty group is tight: `()`, `.{}`.
            let group = Group { row, anchor: self.anchor };
            return next.kind != row.close && group.inside() == Spacing::Spaced;
        }
        if let Some(group) = between.closing {
            return group.inside() == Spacing::Spaced;
        }
        if SEPARATORS.contains(&next.kind) || between.closed == Some(NodeKind::ArrayType) {
            return false;
        }
        // Operators are tight to their operand: `-x`, `&mut x`, `$f`, `..rest`, `x.*`, `a.b`.
        let prefix =
            matches!(self.anchor, Some(NodeKind::Prefix | NodeKind::Spread)) && !self.kind.is_keyword();
        let dot = |t: Self| {
            t.kind == TokenKind::Dot
                && matches!(
                    t.anchor,
                    Some(NodeKind::Member | NodeKind::DotName | NodeKind::Asm | NodeKind::Suffix)
                )
        };
        if prefix
            || dot(self)
            || matches!(next.anchor, Some(NodeKind::Postfix | NodeKind::Writeback))
            || (dot(next) && matches!(next.anchor, Some(NodeKind::Member | NodeKind::Asm | NodeKind::Suffix)))
        {
            return false;
        }
        // A call, an index, a typed literal, a selection and a flow's register open right after
        // what they apply to: `f(x)`, `a[i]`, `Point{ }`, `..std.{ fmt }`, `{in(rdi) p}`.
        let applied = matches!(
            next.anchor,
            Some(NodeKind::Call | NodeKind::Index | NodeKind::TypedLit | NodeKind::Select)
        ) || self.anchor == Some(NodeKind::Flow);
        if next.kind.opens() && applied {
            return false;
        }
        // An infix level may be tight to its operands: `0..n`, and `0.. {` without a right one.
        let tight = |t: Self| {
            t.anchor == Some(NodeKind::Infix) && t.kind.infix().is_some_and(|i| i.around == Spacing::Tight)
        };
        !(tight(next) || (tight(self) && !self.open))
    }
}
