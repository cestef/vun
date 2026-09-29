//! The outline of a file: its declarations, read from the syntax tree alone so that a file that
//! does not check still has one, each with those it holds: a struct's fields, an enum's variants
//! and the members of its namespace.

use vun_grammar::{NodeField, NodeKind};
use vun_span::{FileId, Span};
use vun_syntax::cst::{Cst, NodeId};
use vun_syntax::tokens::Tokens;

use crate::Session;

/// A declaration of the outline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    /// The whole declaration.
    pub span: Span,
    /// Its name.
    pub name_span: Span,
    pub children: Vec<Symbol>,
}

/// What a declaration declares, as its value shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Variant,
    Field,
    Constant,
    Variable,
}

/// The tree of a file and its tokens, read for its outline.
struct Reader<'a> {
    cst: &'a Cst,
    tokens: &'a Tokens,
    text: &'a str,
}

impl Session {
    /// The declarations of `file`, in source order.
    pub fn outline(&mut self, file: FileId) -> Vec<Symbol> {
        let parsed = self.syntax(file);
        let reader = Reader { cst: &parsed.cst, tokens: &parsed.tokens, text: self.text(file) };
        reader.members(reader.cst.root(), NodeField::Items)
    }
}

impl Reader<'_> {
    fn span(&self, n: NodeId) -> Span {
        self.cst.span(n, self.tokens)
    }

    /// The declarations among the nodes of `field` of `n`.
    fn members(&self, n: NodeId, field: NodeField) -> Vec<Symbol> {
        let Some(fields) = self.cst.fields(n) else { return Vec::new() };
        fields.get(field).iter().filter_map(|&entry| self.declaration(entry)).collect()
    }

    /// The node documentation and attributes wrap.
    fn unwrapped(&self, mut n: NodeId) -> NodeId {
        while matches!(self.cst.kind(n), NodeKind::Doc | NodeKind::Attr) {
            match self.cst.fields(n).and_then(|f| f.one(NodeField::Target)) {
                Some(target) => n = target,
                None => break,
            }
        }
        n
    }

    /// The name a pattern binds, when it binds one: `x`, or `$x`.
    fn name(&self, pattern: NodeId) -> Option<NodeId> {
        match self.cst.kind(pattern) {
            NodeKind::Name => Some(pattern),
            NodeKind::Prefix => self.name(self.cst.fields(pattern)?.one(NodeField::Operand)?),
            _ => None,
        }
    }

    fn text_of(&self, n: NodeId) -> String {
        self.span(n).range.of(self.text).to_owned()
    }

    fn declaration(&self, entry: NodeId) -> Option<Symbol> {
        let n = self.unwrapped(entry);
        let kind = self.cst.kind(n);
        if !matches!(kind, NodeKind::Binding | NodeKind::TypedBinding | NodeKind::Field) {
            return None;
        }
        let fields = self.cst.fields(n)?;
        let name = self.name(fields.one(NodeField::Pattern)?)?;
        let value = fields.one(NodeField::Value).map(|v| (v, self.cst.kind(v)));
        let (kind, children) = match value {
            Some((_, NodeKind::Fn | NodeKind::Closure)) => (SymbolKind::Function, Vec::new()),
            Some((v, NodeKind::Struct)) => (SymbolKind::Struct, self.members(v, NodeField::Members)),
            Some((v, NodeKind::Enum)) => (SymbolKind::Enum, self.enumeration(v)),
            _ if kind == NodeKind::Field => (SymbolKind::Field, Vec::new()),
            _ if fields.one(NodeField::Mut).is_some() => (SymbolKind::Variable, Vec::new()),
            _ => (SymbolKind::Constant, Vec::new()),
        };
        let (span, name_span) = (self.span(entry), self.span(name));
        Some(Symbol { name: self.text_of(name), kind, span, name_span, children })
    }

    /// An enum's variants, then the members of its namespace.
    fn enumeration(&self, n: NodeId) -> Vec<Symbol> {
        let Some(fields) = self.cst.fields(n) else { return Vec::new() };
        let variants = fields.get(NodeField::Variants).iter().filter_map(|&v| {
            let v = self.unwrapped(v);
            let name = match self.cst.kind(v) {
                NodeKind::Name => v,
                NodeKind::Call => self.name(self.cst.fields(v)?.one(NodeField::Callee)?)?,
                NodeKind::FieldInit => self.name(self.cst.fields(v)?.one(NodeField::Target)?)?,
                _ => return None,
            };
            let (span, name_span) = (self.span(v), self.span(name));
            Some(Symbol {
                name: self.text_of(name),
                kind: SymbolKind::Variant,
                span,
                name_span,
                children: Vec::new(),
            })
        });
        let namespace = fields.one(NodeField::Namespace).map(|ns| self.members(ns, NodeField::Members));
        variants.chain(namespace.into_iter().flatten()).collect()
    }
}
