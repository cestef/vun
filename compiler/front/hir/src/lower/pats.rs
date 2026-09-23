//! Patterns, from expressions in pattern position: they mirror constructors (spec 01, section 6).

use overt_grammar::{NodeField, NodeKind, TokenKind};
use overt_syntax::cst::{Fields, NodeId};

use super::{Lower, is_filler};
use crate::tree::{Binder, BinderId, Name, Pat, PatId, PatKind};

impl Lower<'_, '_> {
    /// The pattern `n`; `mutable` applies to the binders it introduces.
    pub(super) fn pattern(&mut self, n: NodeId, mutable: bool) -> PatId {
        let kind = self.pattern_kind(n, mutable);
        let origin = self.origin(n);
        self.hir.pats.push(Pat { kind, origin })
    }

    /// The pattern in `field` of `n`, or an `Error` standing for it.
    pub(super) fn pat_at(&mut self, f: &Fields, n: NodeId, field: NodeField, mutable: bool) -> PatId {
        match f.one(field) {
            Some(p) => self.pattern(p, mutable),
            None => self.error_pat(n),
        }
    }

    pub(super) fn error_pat(&mut self, n: NodeId) -> PatId {
        let origin = self.origin(n);
        self.hir.pats.push(Pat { kind: PatKind::Error, origin })
    }

    fn binder(&mut self, n: NodeId, mutable: bool, comptime: bool) -> BinderId {
        let name = self.symbol(n);
        let origin = self.origin(n);
        self.hir.binders.push(Binder { name, mutable, comptime, origin })
    }

    fn bind(&mut self, n: NodeId, mutable: bool, comptime: bool) -> PatKind {
        PatKind::Bind(self.binder(n, mutable, comptime))
    }

    /// The alternatives of `p | q | r`, flattened.
    fn alternatives(&mut self, n: NodeId, mutable: bool, out: &mut Vec<PatId>) {
        if self.kind(n) == NodeKind::Infix && self.token(n) == Some(TokenKind::Pipe) {
            let f = self.fields(n);
            for side in [f.one(NodeField::Left), f.one(NodeField::Right)].into_iter().flatten() {
                self.alternatives(side, mutable, out);
            }
        } else {
            out.push(self.pattern(n, mutable));
        }
    }

    fn pattern_kind(&mut self, n: NodeId, mutable: bool) -> PatKind {
        let kind = self.kind(n);
        let f = if kind.is_leaf() { Fields::default() } else { self.fields(n) };
        match kind {
            NodeKind::Name => self.bind(n, mutable, false),
            NodeKind::Hole => PatKind::Hole,
            NodeKind::Rest => PatKind::Rest(None),
            NodeKind::Prefix if self.token(n) == Some(TokenKind::DotDot) => match f.one(NodeField::Operand) {
                Some(o) if self.kind(o) == NodeKind::Name => {
                    PatKind::Rest(Some(self.binder(o, mutable, false)))
                }
                _ => PatKind::Value(self.expr(n)),
            },
            NodeKind::Binding => match (f.one(NodeField::Pattern), f.one(NodeField::Value)) {
                (Some(name), Some(inner)) if self.kind(name) == NodeKind::Name => {
                    let binder = self.binder(name, mutable, false);
                    PatKind::At { binder, pat: self.pattern(inner, mutable) }
                }
                _ => PatKind::Error,
            },
            NodeKind::Infix if self.token(n) == Some(TokenKind::Pipe) => {
                let mut alternatives = Vec::new();
                self.alternatives(n, mutable, &mut alternatives);
                PatKind::Or(alternatives)
            }
            NodeKind::Infix if matches!(self.token(n), Some(TokenKind::DotDot | TokenKind::DotDotEq)) => {
                let lo = self.operand(&f, n, NodeField::Left);
                let hi = f.one(NodeField::Right).map(|r| self.expr(r));
                PatKind::Range { lo, hi, inclusive: self.token(n) == Some(TokenKind::DotDotEq) }
            }
            NodeKind::Error => PatKind::Error,
            NodeKind::Prefix if self.token(n) == Some(TokenKind::Dollar) => match f.one(NodeField::Operand) {
                Some(o) if self.kind(o) == NodeKind::Name => self.bind(o, mutable, true),
                _ => PatKind::Value(self.expr(n)),
            },
            NodeKind::Paren => match f.one(NodeField::Inner) {
                Some(inner) => self.pattern_kind(inner, mutable),
                None => PatKind::Error,
            },
            NodeKind::Tuple => {
                let items: Vec<_> = f
                    .get(NodeField::Items)
                    .to_vec()
                    .into_iter()
                    .filter(|&i| !is_filler(self.kind(i)))
                    .collect();
                PatKind::Tuple(self.entries(items, &mut |this, i| Some(this.pattern(i, mutable))))
            }
            NodeKind::DotName => PatKind::Variant { name: self.field_name(n), args: None },
            NodeKind::Call if f.one(NodeField::Callee).is_some_and(|c| self.kind(c) == NodeKind::DotName) => {
                let name = f.one(NodeField::Callee).map_or(Name::Hole, |c| self.field_name(c));
                let args = f.get(NodeField::Args).to_vec();
                PatKind::Variant {
                    name,
                    args: Some(self.entries(args, &mut |this, a| Some(this.pattern(a, mutable)))),
                }
            }
            NodeKind::StructLit | NodeKind::TypedLit => {
                let ty = f.one(NodeField::Type).map(|t| self.expr(t));
                let fields = f.get(NodeField::Fields).to_vec();
                let fields = self.entries(fields, &mut |this, f| {
                    Some(if this.kind(f) == NodeKind::FieldInit {
                        let ff = this.fields(f);
                        let name = ff.one(NodeField::Target).map(|t| this.field_name(t));
                        let pat = this.pat_at(&ff, f, NodeField::Value, mutable);
                        (name, pat)
                    } else {
                        (None, this.pattern(f, mutable))
                    })
                });
                PatKind::Struct { ty, fields }
            }
            _ => PatKind::Value(self.expr(n)),
        }
    }
}
