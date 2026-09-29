//! Reading a file's public declarations from its concrete syntax tree: no name is resolved and
//! nothing is evaluated, so a file documents as soon as it parses.

use vun_grammar::{NodeField, NodeKind};
use vun_span::ByteRange;
use vun_syntax::cst::{Anchor, Cst, NodeId};
use vun_syntax::tokens::Tokens;

use crate::item::{Item, Kind};

/// Marks a private declaration, which is not documented.
const PRIVATE: char = '_';
/// Stands for the members of a type in its signature; they are documented below it.
const ELIDED: &str = "{ ... }";

pub(crate) struct Reader<'a> {
    pub(crate) tokens: &'a Tokens,
    pub(crate) cst: &'a Cst,
    pub(crate) text: &'a str,
}

/// Where a declaration is: a field is only a field inside a `struct`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Within {
    Namespace,
    Struct,
}

impl Reader<'_> {
    /// The file's doc comment and its public declarations.
    pub(crate) fn file(&self) -> (Vec<String>, Vec<Item>) {
        let root = self.cst.root();
        let module = self.cst.children(root).filter(|&n| self.cst.kind(n) == NodeKind::ModuleDoc);
        let doc = module.filter_map(|n| self.doc_line(n)).collect();
        let items = self.fields(root, NodeField::Members);
        (doc, self.entries(&items, Within::Namespace))
    }

    fn entries(&self, nodes: &[NodeId], within: Within) -> Vec<Item> {
        nodes.iter().filter_map(|&n| self.entry(n, within)).collect()
    }

    /// The declaration `n`, looking through the doc comments and attributes wrapping it.
    fn entry(&self, mut n: NodeId, within: Within) -> Option<Item> {
        let (mut doc, mut attributes) = (Vec::new(), Vec::new());
        loop {
            match self.cst.kind(n) {
                NodeKind::Doc => doc.push(self.doc_line(n)?),
                NodeKind::Attr => attributes.push(self.attribute(n)),
                _ => break,
            }
            n = self.cst.fields(n)?.one(NodeField::Target)?;
        }
        let mut item = self.decl(n, within)?;
        (item.doc, item.attributes) = (doc, attributes);
        Some(item)
    }

    fn decl(&self, n: NodeId, within: Within) -> Option<Item> {
        if !matches!(self.cst.kind(n), NodeKind::Binding | NodeKind::TypedBinding | NodeKind::Field) {
            return self.variant(n);
        }
        let fields = self.cst.fields(n)?;
        let name = self.name(fields.one(NodeField::Pattern)?)?;
        let value = fields.one(NodeField::Value);
        let ty = fields.one(NodeField::Type);
        let made = value.and_then(|v| self.made(v));
        let (kind, members, elided) = match made {
            Some((NodeKind::Struct, body)) => {
                (Kind::Struct, self.entries(&self.fields(body, NodeField::Members), Within::Struct), body)
            }
            Some((_, body)) => {
                (Kind::Enum, self.entries(&self.fields(body, NodeField::Variants), within), body)
            }
            None => {
                let function = value.map_or_else(
                    || ty.is_some_and(|t| self.function_type(t)),
                    |v| self.cst.kind(v) == NodeKind::Fn || ty.is_some_and(|t| self.function_type(t)),
                );
                let kind = match (function, within) {
                    (true, _) => Kind::Function,
                    (false, Within::Struct) => Kind::Field,
                    (false, Within::Namespace) => Kind::Value,
                };
                let body = value.filter(|&v| self.cst.kind(v) == NodeKind::Fn);
                let block = body.and_then(|f| self.cst.fields(f)?.one(NodeField::Block));
                return Some(self.item(n, name, kind, Vec::new(), block.map(|b| (self.range(b), ""))));
            }
        };
        Some(self.item(n, name, kind, members, self.braces(elided).map(|b| (b, ELIDED))))
    }

    /// A variant of an `enum`: a name, or a name with its payload.
    fn variant(&self, n: NodeId) -> Option<Item> {
        let name = match self.cst.kind(n) {
            NodeKind::Name => self.name(n)?,
            NodeKind::Call => self.name(self.cst.fields(n)?.one(NodeField::Callee)?)?,
            _ => return None,
        };
        Some(self.item(n, name, Kind::Variant, Vec::new(), None))
    }

    /// The item declared by `n`, its signature the source of `n` with `elided` replaced.
    fn item(
        &self,
        n: NodeId,
        name: &str,
        kind: Kind,
        members: Vec<Item>,
        elided: Option<(ByteRange, &str)>,
    ) -> Item {
        let whole = self.range(n);
        let mut signature = String::new();
        match elided {
            Some((cut, with)) if whole.contains(cut) => {
                let before = ByteRange::new(whole.lo, cut.lo).of(self.text);
                let after = ByteRange::new(cut.hi, whole.hi).of(self.text);
                signature.extend([before, with, after]);
            }
            _ => signature.push_str(whole.of(self.text)),
        }
        let line =
            u32::try_from(self.text[..whole.lo as usize].matches('\n').count() + 1).unwrap_or(u32::MAX);
        let signature = signature.split_whitespace().collect::<Vec<_>>().join(" ");
        Item {
            name: name.to_owned(),
            kind,
            doc: Vec::new(),
            attributes: Vec::new(),
            signature,
            members,
            line,
        }
    }

    /// The `struct` or `enum` literal a value is or makes: the literal itself, the body of a
    /// function that only makes one (a type constructor), or the argument of a call that is one
    /// of those (an interface).
    fn made(&self, v: NodeId) -> Option<(NodeKind, NodeId)> {
        match self.cst.kind(v) {
            kind @ (NodeKind::Struct | NodeKind::Enum) => Some((kind, v)),
            NodeKind::Fn => {
                let block = self.cst.fields(v)?.one(NodeField::Block)?;
                match self.fields(block, NodeField::Statements).as_slice() {
                    [only] => self.made(*only),
                    _ => None,
                }
            }
            NodeKind::Call => match self.fields(v, NodeField::Args).as_slice() {
                [only] if self.cst.kind(*only) == NodeKind::Fn => self.made(*only),
                _ => None,
            },
            _ => None,
        }
    }

    /// Whether a type is a function's, possibly `unsafe`.
    fn function_type(&self, t: NodeId) -> bool {
        match self.cst.kind(t) {
            NodeKind::FnType => true,
            NodeKind::Prefix => self
                .cst
                .fields(t)
                .and_then(|f| f.one(NodeField::Operand))
                .is_some_and(|o| self.function_type(o)),
            _ => false,
        }
    }

    /// The braces of a `struct` or `enum` literal and what they hold: every child, the keyword
    /// being the literal's own token.
    fn braces(&self, literal: NodeId) -> Option<ByteRange> {
        self.cst.children(literal).map(|c| self.range(c)).reduce(ByteRange::cover)
    }

    fn fields(&self, n: NodeId, field: NodeField) -> Vec<NodeId> {
        self.cst.fields(n).map(|f| f.get(field).to_vec()).unwrap_or_default()
    }

    fn range(&self, n: NodeId) -> ByteRange {
        self.cst.span(n, self.tokens).range
    }

    fn doc_line(&self, n: NodeId) -> Option<String> {
        match self.cst.anchor(n) {
            Anchor::Token(t) => self.tokens.doc_text(t, self.text).map(str::to_owned),
            Anchor::Missing(_) => None,
        }
    }

    /// An attribute as written, without what it wraps.
    fn attribute(&self, n: NodeId) -> String {
        let target = self.cst.fields(n).and_then(|f| f.one(NodeField::Target));
        let whole = self.range(n);
        let end = target.map_or(whole.hi, |t| self.range(t).lo);
        ByteRange::new(whole.lo, end).of(self.text).trim_end().to_owned()
    }

    /// A public name leaf, without the escape of `\name`.
    fn name(&self, n: NodeId) -> Option<&str> {
        let Anchor::Token(t) = self.cst.anchor(n) else { return None };
        let text = self.tokens.range(t).of(self.text);
        let name = text.strip_prefix('\\').unwrap_or(text);
        (self.cst.kind(n) == NodeKind::Name && !name.starts_with(PRIVATE)).then_some(name)
    }
}
