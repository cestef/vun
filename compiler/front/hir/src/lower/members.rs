//! Namespaces, declarations, fields, variants and blocks.

use vun_grammar::{NodeField, NodeKind};
use vun_syntax::cst::{Fields, NodeId};

use super::Lower;
use super::pats::Mode;
use crate::tree::{
    Block, BlockId, Decl, DeclId, ExprId, FieldDef, List, Member, Name, Namespace, NsId, Spread, Stmt,
    Variant,
};

impl Lower<'_, '_> {
    /// The members of a file or a `struct` body.
    pub(super) fn members(&mut self, nodes: impl IntoIterator<Item = NodeId>) -> List<Member> {
        let nodes: Vec<_> = nodes.into_iter().filter(|&n| !self.filler(n)).collect();
        self.entries(nodes, &mut |this, n| this.member(n))
    }

    fn member(&mut self, n: NodeId) -> Option<Member> {
        let (inner, doc, attrs) = self.annotations(n);
        match self.kind(inner) {
            NodeKind::Binding | NodeKind::TypedBinding => Some(Member::Decl(self.decl(inner, doc, attrs))),
            NodeKind::Field => Some(Member::Field(self.field(inner, doc, attrs))),
            NodeKind::Spread => Some(Member::Spread(self.spread(inner))),
            _ => None,
        }
    }

    pub(super) fn namespace(&mut self, f: &Fields, n: NodeId) -> NsId {
        let members = self.members(f.get(NodeField::Members).to_vec());
        let origin = self.origin(n);
        self.hir.namespaces.push(Namespace { members, origin })
    }

    /// The members of an `enum` literal: its variants, then the declarations after `;`, which are
    /// its namespace.
    pub(super) fn enum_body(&mut self, f: &Fields, n: NodeId) -> (List<Variant>, NsId) {
        let (variants, members): (Vec<NodeId>, Vec<NodeId>) =
            f.get(NodeField::Members).iter().partition(|&&c| self.is_variant(c));
        let variants = self.variants(variants);
        let members = self.members(members);
        let origin = self.origin(n);
        (variants, self.hir.namespaces.push(Namespace { members, origin }))
    }

    /// Whether a member node of an `enum` is a variant, `name` or `name(T)`, through its docs.
    fn is_variant(&self, n: NodeId) -> bool {
        let mut inner = n;
        while matches!(self.kind(inner), NodeKind::Doc | NodeKind::Attr) {
            let Some(target) = self.fields(inner).one(NodeField::Target) else { return false };
            inner = target;
        }
        matches!(self.kind(inner), NodeKind::Name | NodeKind::Call) || self.staged_control(inner).is_some()
    }

    pub(super) fn decl(&mut self, n: NodeId, doc: Vec<String>, attrs: Vec<ExprId>) -> DeclId {
        let f = self.fields(n);
        let mutable = f.one(NodeField::Mut).is_some();
        let ((pat, ty, value), origin) = self.declaring(n, f.one(NodeField::Pattern), |this| {
            let pat = this.pat_at(&f, n, NodeField::Pattern, Mode { mutable, ..Mode::PLAIN });
            let ty = f.one(NodeField::Type).map(|t| this.expr(t));
            (pat, ty, f.one(NodeField::Value).map(|v| this.expr(v)))
        });
        self.hir.decls.push(Decl { pat, ty, value, attrs, doc, origin })
    }

    fn spread(&mut self, n: NodeId) -> Spread {
        let f = self.fields(n);
        let path = self.operand(&f, n, NodeField::Source);
        let select = f.one(NodeField::Select).map(|s| {
            let mut names = self.fields(s).get(NodeField::Names).to_vec();
            names.retain(|&n| self.kind(n) == NodeKind::Name);
            names.into_iter().map(|n| self.binder(n, Mode::PLAIN)).collect()
        });
        Spread { path, select, origin: self.origin(n) }
    }

    fn field(&mut self, n: NodeId, doc: Vec<String>, attrs: Vec<ExprId>) -> FieldDef {
        let f = self.fields(n);
        let mutable = f.one(NodeField::Mut).is_some();
        let name = f.one(NodeField::Pattern).map_or(Name::Hole, |p| self.name(p));
        let ((ty, default), origin) = self.declaring(n, f.one(NodeField::Pattern), |this| {
            (this.operand(&f, n, NodeField::Type), f.one(NodeField::Value).map(|v| this.expr(v)))
        });
        FieldDef { name, mutable, ty, default, attrs, doc, origin }
    }

    fn variants(&mut self, nodes: Vec<NodeId>) -> List<Variant> {
        self.entries(nodes, &mut |this, n| {
            let (inner, doc, _) = this.annotations(n);
            let origin = this.origin(inner);
            match this.kind(inner) {
                NodeKind::Name => Some(Variant { name: this.name(inner), payload: Vec::new(), doc, origin }),
                NodeKind::Call => {
                    let f = this.fields(inner);
                    let name = f.one(NodeField::Callee).map_or(Name::Hole, |c| this.name(c));
                    let payload = f.get(NodeField::Args).iter().map(|&a| this.expr(a)).collect();
                    Some(Variant { name, payload, doc, origin })
                }
                _ => None,
            }
        })
    }

    /// The block in `field`; an empty block standing for `n` when absent.
    pub(super) fn block_at(&mut self, f: &Fields, n: NodeId, field: NodeField) -> BlockId {
        self.block(f.one(field).unwrap_or(n))
    }

    /// A `{ }` block or a `#{ }` quote: its statements, and the last expression as its value when
    /// no `;` follows it.
    pub(super) fn block(&mut self, n: NodeId) -> BlockId {
        let origin = self.origin(n);
        if !matches!(self.kind(n), NodeKind::Block | NodeKind::Quote) {
            return self.hir.blocks.push(Block { stmts: Vec::new(), tail: None, origin });
        }
        // The tail is the last child before the closing brace, when it is an expression.
        let last = self.source.cst.children_rev(n).nth(1);
        let tail = last.filter(|&c| !self.filler(c) && self.is_value(c));
        let statements: Vec<_> =
            self.fields(n).get(NodeField::Statements).iter().copied().filter(|&c| Some(c) != tail).collect();
        let stmts = self.entries(statements, &mut |this, s| Some(this.stmt(s)));
        let tail = tail.map(|t| self.expr(t));
        self.hir.blocks.push(Block { stmts, tail, origin })
    }

    /// Whether a statement node is an expression, which can be a block's value, attributes and
    /// docs included: `{ #[kind.linear] struct { ... } }`. A `$for` generates statements, even
    /// last (spec 03, section 1).
    fn is_value(&self, n: NodeId) -> bool {
        let mut inner = n;
        while matches!(self.kind(inner), NodeKind::Doc | NodeKind::Attr) {
            let Some(target) = self.fields(inner).one(NodeField::Target) else { return false };
            inner = target;
        }
        self.staged_control(inner).is_none_or(|c| self.kind(c) != NodeKind::For)
            && !matches!(self.kind(inner), NodeKind::Binding | NodeKind::TypedBinding | NodeKind::Assign)
    }

    fn stmt(&mut self, n: NodeId) -> Stmt {
        let (inner, doc, attrs) = self.annotations(n);
        match self.kind(inner) {
            NodeKind::Binding | NodeKind::TypedBinding => Stmt::Decl(self.decl(inner, doc, attrs)),
            NodeKind::Assign => self.assignment(inner),
            _ => Stmt::Expr(self.expr(inner)),
        }
    }
}
