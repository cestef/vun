//! Lowering a file's CST into HIR, through the named children of each node (`Cst::fields`).
//!
//! Lowering decides roles (pattern or expression, member or statement) from positions and reports
//! nothing: the parser reported malformed source, which lowers to `Error` nodes, and the checks
//! that need the HIR run after it.

mod control;
mod exprs;
mod members;
mod pats;

use std::collections::HashMap;

use vun_grammar::{NodeField, NodeKind, TokenKind};
use vun_intern::Symbol;
use vun_origin::{OriginId, OriginTable, Via};
use vun_syntax::cst::{Anchor, Cst, Fields, NodeId};
use vun_syntax::tokens::Tokens;

use crate::tree::{Entry, Hir, List, Name, Staged};
use pats::Mode;

/// A parsed file, as lowering reads it.
#[derive(Clone, Copy)]
pub struct Source<'a> {
    pub cst: &'a Cst,
    pub tokens: &'a Tokens,
    pub text: &'a str,
}

impl Hir {
    /// Lowers the file `source` into HIR.
    #[must_use]
    pub fn lower(source: Source<'_>, origins: &OriginTable) -> Self {
        let mut lower =
            Lower { source, origins, hir: Self::default(), declaration: None, made: HashMap::new() };
        let root = source.cst.root();
        let members = lower.members(source.cst.children(root));
        let origin = lower.origin(root);
        lower.hir.root = Some(lower.hir.namespaces.push(crate::tree::Namespace { members, origin }));
        lower.hir
    }
}

struct Lower<'a, 'i> {
    source: Source<'a>,
    origins: &'i OriginTable,
    hir: Hir,
    /// The origin of the declaration being lowered, which the origins inside it are anchored to.
    declaration: Option<OriginId>,
    /// How many entities have each origin so far: a further one gets its own.
    made: HashMap<OriginId, u16>,
}

/// Escape that makes a keyword a name: `\unsafe`.
const ESCAPE: char = '\\';
/// Prefix of labels: `'outer`.
const LABEL: char = '\'';

impl<'a> Lower<'a, '_> {
    /// The origin of `n`: inside the declaration being lowered, or in the file.
    fn origin(&mut self, n: NodeId) -> OriginId {
        let span = self.source.cst.span(n, self.source.tokens);
        let origin = match self.declaration {
            Some(declaration) => self.origins.inside(declaration, span),
            None => self.origins.source(span),
        };
        self.own(origin)
    }

    /// `origin` for one more entity: itself for the first, then, since two entities made from
    /// one piece of syntax are two, an origin derived from it for each further one, by its order.
    fn own(&mut self, origin: OriginId) -> OriginId {
        let made = self.made.entry(origin).or_insert(0);
        let nth = *made;
        *made += 1;
        match nth {
            0 => origin,
            _ => self.origins.derive(origin, self.origins.get(origin).span, Via::Again(nth)),
        }
    }

    /// Lowers the declaration `n` with `lower`, anchoring the origins inside it to the returned
    /// origin: the declaration's name when `name` is a plain name, else its place.
    fn declaring<T>(
        &mut self,
        n: NodeId,
        name: Option<NodeId>,
        lower: impl FnOnce(&mut Self) -> T,
    ) -> (T, OriginId) {
        let name = name.filter(|&p| self.kind(p) == NodeKind::Name).map(|p| self.symbol(p));
        let origin = match name {
            Some(name) => {
                let span = self.source.cst.span(n, self.source.tokens);
                let origin = self.origins.declaration(span, self.declaration, name);
                self.own(origin)
            }
            None => self.origin(n),
        };
        let outer = self.declaration.replace(origin);
        let lowered = lower(self);
        self.declaration = outer;
        (lowered, origin)
    }

    /// The text of the doc comment `n` without `///` and the space after it.
    fn doc_line(&self, n: NodeId) -> String {
        const DOC: &str = "///";
        let text = self.text(n);
        let line = text.strip_prefix(DOC).unwrap_or(text);
        line.strip_prefix(' ').unwrap_or(line).to_owned()
    }

    /// Whether `n` stands for nothing in a list: a separator or a module doc.
    fn filler(&self, n: NodeId) -> bool {
        matches!(self.kind(n), NodeKind::Punct | NodeKind::ModuleDoc)
    }

    fn kind(&self, n: NodeId) -> NodeKind {
        self.source.cst.kind(n)
    }

    /// The token anchoring `n`, if present in the source.
    fn token(&self, n: NodeId) -> Option<TokenKind> {
        match self.source.cst.anchor(n) {
            Anchor::Token(t) => Some(self.source.tokens.kind(t)),
            Anchor::Missing(_) => None,
        }
    }

    fn text(&self, n: NodeId) -> &'a str {
        match self.source.cst.anchor(n) {
            Anchor::Token(t) => self.source.tokens.range(t).of(self.source.text),
            Anchor::Missing(_) => "",
        }
    }

    /// The named children of `n`, matched once; lowering fetches them once per node.
    fn fields(&self, n: NodeId) -> Fields {
        self.source.cst.fields(n).unwrap_or_default()
    }

    /// The symbol of a name leaf, without the escape of `\name`.
    fn symbol(&mut self, n: NodeId) -> Symbol {
        let text = self.text(n);
        Symbol::intern(text.strip_prefix(ESCAPE).unwrap_or(text))
    }

    fn label(&mut self, n: NodeId) -> Symbol {
        let text = self.text(n);
        Symbol::intern(text.strip_prefix(LABEL).unwrap_or(text))
    }

    /// A name in a name position: plain, `$(e)`, a tuple index, or `_`.
    fn name(&mut self, n: NodeId) -> Name {
        match self.kind(n) {
            NodeKind::Name => Name::Plain(self.symbol(n)),
            NodeKind::Int => vun_syntax::literal::integer(self.text(n))
                .and_then(|i| u32::try_from(i).ok())
                .map_or(Name::Hole, Name::Index),
            NodeKind::Prefix if self.token(n) == Some(TokenKind::Dollar) => {
                // Parentheses lower to their inner expression.
                let f = self.fields(n);
                Name::Computed(self.operand(&f, n, NodeField::Operand))
            }
            _ => Name::Hole,
        }
    }

    /// Doc lines and attribute values wrapping `n`, and the node they wrap.
    fn annotations(&mut self, mut n: NodeId) -> (NodeId, Vec<String>, Vec<crate::tree::ExprId>) {
        let (mut doc, mut attrs) = (Vec::new(), Vec::new());
        loop {
            match self.kind(n) {
                NodeKind::Doc | NodeKind::Attr => {}
                _ => return (n, doc, attrs),
            }
            let f = self.fields(n);
            if self.kind(n) == NodeKind::Doc {
                doc.push(self.doc_line(n));
            }
            for &value in f.get(NodeField::Values) {
                attrs.push(self.expr(value));
            }
            match f.one(NodeField::Target) {
                Some(target) => n = target,
                None => return (n, doc, attrs),
            }
        }
    }

    /// The entries of a list, where `$for` and `$if` produce entries of the same list.
    fn entries<T>(
        &mut self,
        nodes: impl IntoIterator<Item = NodeId>,
        one: &mut dyn FnMut(&mut Self, NodeId) -> Option<T>,
    ) -> List<T> {
        let mut list = Vec::new();
        for n in nodes {
            match self.staged(n, one) {
                Some(staged) => list.push(Entry::Staged(staged)),
                None => list.extend(one(self, n).map(Entry::One)),
            }
        }
        list
    }

    /// `$for p in e { entries }` or `$if c { entries } else { entries }` in a list.
    fn staged<T>(
        &mut self,
        n: NodeId,
        one: &mut dyn FnMut(&mut Self, NodeId) -> Option<T>,
    ) -> Option<Staged<T>> {
        let control = self.staged_control(n)?;
        let origin = self.origin(n);
        match self.kind(control) {
            NodeKind::For => {
                let f = self.fields(control);
                let head = f.one(NodeField::Head).filter(|&h| self.kind(h) == NodeKind::In)?;
                let h = self.fields(head);
                let pat = self.pat_at(&h, head, NodeField::Pattern, Mode { comptime: true, ..Mode::PLAIN });
                let iter = self.operand(&h, head, NodeField::Value);
                let body = self.block_entries(&f, one);
                Some(Staged::For { pat, iter, body, origin })
            }
            NodeKind::If => Some(self.staged_if(control, origin, one)),
            _ => None,
        }
    }

    /// The `for` or `if` of a `$for` or `$if` node.
    fn staged_control(&self, n: NodeId) -> Option<NodeId> {
        if self.kind(n) != NodeKind::Prefix || self.token(n) != Some(TokenKind::Dollar) {
            return None;
        }
        let control = self.fields(n).one(NodeField::Operand)?;
        matches!(self.kind(control), NodeKind::For | NodeKind::If).then_some(control)
    }

    fn staged_if<T>(
        &mut self,
        n: NodeId,
        origin: OriginId,
        one: &mut dyn FnMut(&mut Self, NodeId) -> Option<T>,
    ) -> Staged<T> {
        let f = self.fields(n);
        let cond = self.operand(&f, n, NodeField::Condition);
        let then = self.block_entries(&f, one);
        let otherwise = match f.one(NodeField::Otherwise) {
            // `else if` inside a staged `if` is staged too.
            Some(o) if self.kind(o) == NodeKind::If => vec![Entry::Staged(self.staged_if(o, origin, one))],
            Some(o) => {
                let statements = self.fields(o).get(NodeField::Statements).to_vec();
                self.entries(statements, one)
            }
            None => Vec::new(),
        };
        Staged::If { cond, then, otherwise, origin }
    }

    /// The entries inside the `block` field.
    fn block_entries<T>(
        &mut self,
        f: &Fields,
        one: &mut dyn FnMut(&mut Self, NodeId) -> Option<T>,
    ) -> List<T> {
        let statements = f.one(NodeField::Block).map(|b| self.fields(b).get(NodeField::Statements).to_vec());
        self.entries(statements.unwrap_or_default(), one)
    }
}
