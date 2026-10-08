//! `impl I for T { members }`, `impl T { members }` and `impl I for T = value;` (spec 02, section
//! 3): the declaration of the witness each stands for. It lowers to what `#[impl(I(C))]`, or
//! `#[impl(C)]` without an interface, on a declaration is: a function of the parameters the
//! target binds, `($r) -> type { struct { members } }` for `Int($r)`, or the `struct` alone where
//! the target is a type; the value itself for `= value`. So the witness, its conformance and what
//! is reported of it are those of the longer form. It binds no name: nothing refers to a witness
//! but by the types it is for.

use vun_grammar::{NodeField, NodeKind, TokenKind};
use vun_items::Item;
use vun_origin::OriginId;
use vun_span::Span;
use vun_syntax::cst::{Fields, NodeId};

use super::Lower;
use super::pats::Mode;
use crate::tree::{
    Arg, Block, Decl, DeclId, Entry, Expr, ExprId, ExprKind, Name, Namespace, Param, Pat, PatKind,
};

impl Lower<'_, '_> {
    /// The declaration the `impl` node `n`, of any of its kinds, stands for.
    pub(super) fn implementation(&mut self, n: NodeId, doc: Vec<String>, mut attrs: Vec<ExprId>) -> DeclId {
        let f = self.fields(n);
        let target = f.one(NodeField::Target);
        let (constructor, binders) = target.map_or((None, Vec::new()), |t| self.target(t));
        let span = self.head_span(n, &f);
        let ((pat, value, registers), origin) = self.declared(n, None, |this| {
            let head = this.origin_at(span);
            let registers = this.registers(&f, constructor.or(target), head);
            let body = if let Some(value) = f.one(NodeField::Value) {
                this.expr(value)
            } else {
                let members = this.members(f.get(NodeField::Members).to_vec());
                let ns = Namespace { members, origin: this.origin(n) };
                let ns = this.hir.namespaces.push(ns);
                this.synthesized(ExprKind::Struct(ns), n)
            };
            let value = if binders.is_empty() { body } else { this.witness_fn(&binders, body, n, head) };
            // The declaration binds nothing: no name refers to a witness.
            let pat = this.hir.pats.push(Pat { kind: PatKind::Hole, origin: head });
            (pat, value, registers)
        });
        attrs.insert(0, registers);
        self.hir.decls.push(Decl { pat, ty: None, value: Some(value), attrs, doc, origin })
    }

    /// What a target stands for: the constructor, and the nodes binding its parameters, for
    /// `C($a, $b)`; the target itself, a type, and none otherwise.
    fn target(&self, target: NodeId) -> (Option<NodeId>, Vec<NodeId>) {
        if self.kind(target) == NodeKind::Call {
            let f = self.fields(target);
            let args: Vec<NodeId> =
                f.get(NodeField::Args).iter().copied().filter(|&a| !self.filler(a)).collect();
            if !args.is_empty() && args.iter().all(|&a| self.binds(a)) {
                return (f.one(NodeField::Callee), args);
            }
        }
        (None, Vec::new())
    }

    /// Whether `n` is `$name`, which binds a parameter of the target's constructor.
    fn binds(&self, n: NodeId) -> bool {
        self.kind(n) == NodeKind::Prefix
            && self.token(n) == Some(TokenKind::Dollar)
            && self.fields(n).one(NodeField::Operand).is_some_and(|o| self.kind(o) == NodeKind::Name)
    }

    /// The attribute `lang.impl(I(C))`, with `C` the constructor `target` stands for, or the type;
    /// `lang.impl(C)` where there is no interface.
    fn registers(&mut self, f: &Fields, target: Option<NodeId>, head: OriginId) -> ExprId {
        let implemented = match target {
            Some(t) => self.expr(t),
            None => self.synthesized_at(ExprKind::Error, head),
        };
        let implemented = match f.one(NodeField::Interface) {
            Some(interface) => {
                let interface = self.expr(interface);
                let args = vec![Entry::One(Arg { name: None, value: implemented })];
                self.synthesized_at(ExprKind::Call { callee: interface, args }, head)
            }
            None => implemented,
        };
        let callee = self.item(vun_items::lang::r#impl, head);
        let args = vec![Entry::One(Arg { name: None, value: implemented })];
        self.synthesized_at(ExprKind::Call { callee, args }, head)
    }

    /// The function `($a, $b) -> type { body }` making the witness for each value of the
    /// parameters.
    fn witness_fn(&mut self, binders: &[NodeId], body: ExprId, n: NodeId, head: OriginId) -> ExprId {
        let params = binders
            .iter()
            .map(|&b| {
                let origin = self.origin(b);
                let pat = Some(self.pattern(b, Mode::PLAIN));
                Entry::One(Param { pat, ty: None, attrs: Vec::new(), origin })
            })
            .collect();
        let origin = self.origin(n);
        let body = self.hir.blocks.push(Block { stmts: Vec::new(), tail: Some(body), origin });
        let returns = Some(self.item(vun_items::lang::r#type, head));
        self.synthesized(ExprKind::Fn { params, returns, body }, n)
    }

    /// The path of the `lang` item `item`, written out.
    fn item(&mut self, item: Item, head: OriginId) -> ExprId {
        let kind = match item.parent() {
            Some(parent) => {
                let object = self.item(parent, head);
                ExprKind::Member { object, name: Name::Plain(item.symbol()) }
            }
            None => ExprKind::Name(item.symbol()),
        };
        self.synthesized_at(kind, head)
    }

    fn synthesized(&mut self, kind: ExprKind, n: NodeId) -> ExprId {
        let origin = self.origin(n);
        self.hir.exprs.push(Expr { kind, origin })
    }

    fn synthesized_at(&mut self, kind: ExprKind, at: OriginId) -> ExprId {
        let origin = self.own(at);
        self.hir.exprs.push(Expr { kind, origin })
    }

    /// The span of the head of the `impl` node `n`, from `impl` to the end of its target, where
    /// what is said of the impl points: its body is its members'.
    fn head_span(&self, n: NodeId, f: &Fields) -> Span {
        let whole = self.source.cst.span(n, self.source.tokens);
        let end = f
            .one(NodeField::Target)
            .map_or(whole.range.hi, |t| self.source.cst.span(t, self.source.tokens).range.hi);
        Span::new(whole.file, whole.range.lo, end)
    }
}
