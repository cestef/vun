//! Expressions and types.

use overt_grammar::{NodeField, NodeKind, TokenKind};
use overt_syntax::cst::{Fields, NodeId};
use overt_syntax::lexer;

use super::Lower;
use crate::tree::{Arg, Expr, ExprId, ExprKind, List, Lit, Name};

impl Lower<'_, '_> {
    pub(super) fn expr(&mut self, n: NodeId) -> ExprId {
        let kind = self.expr_kind(n);
        let origin = self.origin(n);
        self.hir.exprs.push(Expr { kind, origin })
    }

    /// An `Error` expression standing for `n`.
    pub(super) fn error(&mut self, n: NodeId) -> ExprId {
        let origin = self.origin(n);
        self.hir.exprs.push(Expr { kind: ExprKind::Error, origin })
    }

    /// The expression in `field`, or an `Error` standing for `n` when it is absent.
    pub(super) fn operand(&mut self, f: &Fields, n: NodeId, field: NodeField) -> ExprId {
        match f.one(field) {
            Some(c) => self.expr(c),
            None => self.error(n),
        }
    }

    fn exprs(&mut self, nodes: &[NodeId]) -> List<ExprId> {
        let nodes: Vec<_> = nodes.iter().copied().filter(|&n| !self.filler(n)).collect();
        self.entries(nodes, &mut |this, n| Some(this.expr(n)))
    }

    /// Arguments and literal fields: `value` or `.name = value`.
    fn args(&mut self, nodes: &[NodeId]) -> List<Arg> {
        self.entries(nodes.to_vec(), &mut |this, n| {
            if this.kind(n) == NodeKind::FieldInit {
                let f = this.fields(n);
                let name = f.one(NodeField::Target).map(|t| this.field_name(t));
                let value = this.operand(&f, n, NodeField::Value);
                Some(Arg { name, value })
            } else {
                Some(Arg { name: None, value: this.expr(n) })
            }
        })
    }

    /// The name of `.name` in a field initializer or pattern.
    pub(super) fn field_name(&mut self, n: NodeId) -> Name {
        match self.kind(n) {
            NodeKind::DotName => self.fields(n).one(NodeField::Property).map_or(Name::Hole, |p| self.name(p)),
            _ => self.name(n),
        }
    }

    fn expr_kind(&mut self, n: NodeId) -> ExprKind {
        let kind = self.kind(n);
        if kind.is_leaf() {
            return self.leaf(n);
        }
        let f = self.fields(n);
        match kind {
            NodeKind::MultilineString => ExprKind::Lit(Lit::String(self.multiline(&f, n))),
            NodeKind::Paren => f.one(NodeField::Inner).map_or(ExprKind::Error, |i| self.expr_kind(i)),
            NodeKind::DotName => {
                ExprKind::DotName(f.one(NodeField::Property).map_or(Name::Hole, |p| self.name(p)))
            }
            NodeKind::Member => {
                let object = self.operand(&f, n, NodeField::Object);
                let name = f.one(NodeField::Property).map_or(Name::Hole, |p| self.name(p));
                ExprKind::Member { object, name }
            }
            NodeKind::Call => {
                let callee = self.operand(&f, n, NodeField::Callee);
                ExprKind::Call { callee, args: self.args(f.get(NodeField::Args)) }
            }
            NodeKind::Index => ExprKind::Index {
                object: self.operand(&f, n, NodeField::Object),
                index: self.operand(&f, n, NodeField::Subscript),
            },
            NodeKind::StructLit => {
                ExprKind::StructLit { ty: None, fields: self.args(f.get(NodeField::Fields)) }
            }
            NodeKind::TypedLit => {
                let ty = Some(self.operand(&f, n, NodeField::Type));
                ExprKind::StructLit { ty, fields: self.args(f.get(NodeField::Fields)) }
            }
            NodeKind::Tuple => ExprKind::Tuple(self.exprs(f.get(NodeField::Items))),
            NodeKind::Array => ExprKind::Array(self.exprs(f.get(NodeField::Items))),
            NodeKind::ArrayRepeat => ExprKind::Repeat {
                value: self.operand(&f, n, NodeField::Value),
                count: self.operand(&f, n, NodeField::Count),
            },
            NodeKind::ArrayType => ExprKind::ArrayType {
                len: self.operand(&f, n, NodeField::Length),
                elem: self.operand(&f, n, NodeField::Element),
            },
            NodeKind::Prefix => {
                let mode = f.one(NodeField::Mode).map(|m| self.symbol(m));
                let operand = self.operand(&f, n, NodeField::Operand);
                self.token(n).map_or(ExprKind::Error, |op| ExprKind::Prefix { op, mode, operand })
            }
            NodeKind::Postfix => {
                let operand = self.operand(&f, n, NodeField::Operand);
                self.token(n).map_or(ExprKind::Error, |op| ExprKind::Postfix { op, operand })
            }
            NodeKind::Infix => {
                let left = self.operand(&f, n, NodeField::Left);
                let right = f.one(NodeField::Right).map(|r| self.expr(r));
                self.token(n).map_or(ExprKind::Error, |op| ExprKind::Infix { op, left, right })
            }
            NodeKind::Block => ExprKind::Block(self.block(n)),
            NodeKind::Quote => ExprKind::Quote(self.block(n)),
            NodeKind::Struct => ExprKind::Struct(self.namespace(&f, n)),
            NodeKind::Enum => {
                let (variants, ns) = self.enum_body(&f, n);
                ExprKind::Enum { variants, ns }
            }
            NodeKind::Doc | NodeKind::Attr => {
                let (inner, _, attrs) = self.annotations(n);
                ExprKind::Attributed { attrs, inner: self.expr(inner) }
            }
            NodeKind::Closure => {
                let captures = self.exprs(f.get(NodeField::Captures));
                ExprKind::Closure { captures, func: self.operand(&f, n, NodeField::Fn) }
            }
            NodeKind::Assign => {
                // An assignment in expression position lowers as a block holding it.
                let origin = self.origin(n);
                let stmt = self.assignment(n);
                ExprKind::Block(self.hir.blocks.push(crate::tree::Block {
                    stmts: vec![crate::tree::Entry::One(stmt)],
                    tail: None,
                    origin,
                }))
            }
            _ => self.control(&f, n),
        }
    }

    fn leaf(&mut self, n: NodeId) -> ExprKind {
        match self.kind(n) {
            NodeKind::Name => ExprKind::Name(self.symbol(n)),
            NodeKind::Hole => ExprKind::Hole,
            NodeKind::Int => overt_syntax::literal::integer(self.text(n))
                .map_or(ExprKind::Error, |i| ExprKind::Lit(Lit::Int(i))),
            NodeKind::Float => ExprKind::Lit(Lit::Float(self.text(n).to_owned())),
            NodeKind::Char => self
                .token(n)
                .and_then(|k| lexer::literal(k, self.text(n)))
                .and_then(|t| t.chars().next())
                .map_or(ExprKind::Error, |c| ExprKind::Lit(Lit::Char(c))),
            NodeKind::String => self
                .token(n)
                .and_then(|k| lexer::literal(k, self.text(n)))
                .map_or(ExprKind::Error, |t| ExprKind::Lit(Lit::String(t.into_owned()))),
            _ => ExprKind::Error,
        }
    }

    pub(super) fn assignment(&mut self, n: NodeId) -> crate::tree::Stmt {
        let f = self.fields(n);
        let op = self.token(n).unwrap_or(TokenKind::Eq);
        let place = self.operand(&f, n, NodeField::Place);
        let value = self.operand(&f, n, NodeField::Value);
        crate::tree::Stmt::Assign { op, place, value, origin: self.origin(n) }
    }

    fn multiline(&mut self, f: &Fields, n: NodeId) -> String {
        let mut lines = vec![self.raw_line(n)];
        for &line in f.get(NodeField::Lines) {
            lines.push(self.raw_line(line));
        }
        lines.join("\n")
    }

    fn raw_line(&self, n: NodeId) -> String {
        self.token(n)
            .and_then(|k| lexer::literal(k, self.text(n)))
            .map(std::borrow::Cow::into_owned)
            .unwrap_or_default()
    }
}
