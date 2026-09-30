//! The editors' facts of `editors.vun`, resolved: what highlights, the outline and text objects
//! capture, and how the Sublime syntax scopes tokens.

use vun_syntax::literal::Name;

use super::paths::Files;
use super::written::{self, WrittenEditors};
use super::{Field, Kind, Model, Token};
use crate::generate::{Read, Source};
use crate::grammar::sublime::Scope;
use crate::grammar::tree_sitter::queries::Capture;

/// A node of a syntax tree as editors name it: of a node kind, or a token kind's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Node {
    Kind(Kind),
    Token(Token),
}

/// A node captured where it stands: in `field` of one of `kinds`, or in `inner` of what that
/// holds.
pub(crate) struct Place {
    pub(crate) kinds: Vec<Kind>,
    pub(crate) field: Field,
    pub(crate) inner: Option<Field>,
    pub(crate) node: Kind,
    pub(crate) capture: Capture,
}

/// Nodes captured whole, and where they stand.
pub(crate) struct Captures {
    pub(crate) nodes: Vec<(Vec<Node>, Capture)>,
    pub(crate) places: Vec<Place>,
}

/// A binding whose value, of one of `values`, says what its name, in `name`, names.
pub(crate) struct Binding {
    pub(crate) kinds: Vec<Kind>,
    pub(crate) name: Field,
    pub(crate) value: Field,
    pub(crate) values: Vec<Kind>,
    pub(crate) capture: Capture,
}

pub(crate) struct Highlights {
    /// The leaf names are.
    pub(crate) name: Kind,
    pub(crate) tokens: Vec<(Vec<Token>, Capture)>,
    pub(crate) captures: Captures,
    pub(crate) bindings: Vec<Binding>,
}

/// What the outline lists: nodes of `kinds`, by the `node` in their `field`.
pub(crate) struct Outline {
    pub(crate) kinds: Vec<Kind>,
    pub(crate) field: Field,
    pub(crate) node: Kind,
}

pub(crate) struct Sublime {
    /// Punctuation scoped by the first table it is in, and apart from them.
    pub(crate) tables: Vec<(Vec<Token>, Scope)>,
    pub(crate) special: Vec<(Token, Scope)>,
    pub(crate) keywords: Vec<Token>,
    pub(crate) qualifiers: Vec<String>,
    pub(crate) modes: Vec<String>,
    pub(crate) punctuation: Vec<Token>,
    /// The tokens placing names: a member after, a call before, a definition before, a borrow's
    /// words after.
    pub(crate) member: Token,
    pub(crate) call: Token,
    pub(crate) binds: Token,
    pub(crate) borrow: Token,
}

/// What the tree-sitter grammar leaves to tree-sitter: the kind of skipped source and the tokens
/// ending a file; the token keywords are spelled as; the tokens standing anywhere.
pub(crate) struct TreeSitter {
    pub(crate) error: Kind,
    pub(crate) end: Vec<Token>,
    pub(crate) word: Token,
    pub(crate) extras: Vec<Token>,
}

pub(crate) struct Editors {
    pub(crate) highlights: Highlights,
    pub(crate) outline: Outline,
    pub(crate) text_objects: Captures,
    pub(crate) sublime: Sublime,
    pub(crate) tree_sitter: TreeSitter,
}

impl Editors {
    /// The editors' facts among `sources`, resolved against the grammar's `model`.
    pub(crate) fn read(model: &Model, sources: &[Source<'_>]) -> Read<Self> {
        let files = Files::of(sources);
        let written = files.read::<WrittenEditors>()?;
        model.editors(&files, &written)
    }
}

impl Model {
    fn editors(&self, files: &Files<'_, '_>, written: &WrittenEditors) -> Read<Editors> {
        let h = &written.highlights;
        let tokens = h.tokens.iter().map(|t| Ok((self.tokens_of(files, &t.tokens)?, t.capture)));
        let highlights = Highlights {
            name: self.kind_named("highlights", &h.name)?,
            tokens: tokens.collect::<Read<_>>()?,
            captures: self.captures(&h.nodes, &h.places)?,
            bindings: h
                .bindings
                .iter()
                .map(|b| {
                    Ok(Binding {
                        kinds: self.kinds_named("highlights", &b.kinds)?,
                        name: self.field_named("highlights", &b.name)?,
                        value: self.field_named("highlights", &b.value)?,
                        values: self.kinds_named("highlights", &b.values)?,
                        capture: b.capture,
                    })
                })
                .collect::<Read<_>>()?,
        };
        let o = &written.outline;
        let outline = Outline {
            kinds: self.kinds_named("outline", &o.kinds)?,
            field: self.field_named("outline", &o.field)?,
            node: self.kind_named("outline", &o.node)?,
        };
        let s = &written.sublime;
        let token = |name: &Name| self.token_named("sublime", name);
        let sublime = Sublime {
            tables: s
                .tables
                .iter()
                .map(|t| Ok((self.tokens_of(files, &t.tokens)?, t.scope)))
                .collect::<Read<_>>()?,
            special: s
                .special
                .iter()
                .map(|t| Ok((self.token_named("sublime", &t.token)?, t.scope)))
                .collect::<Read<_>>()?,
            keywords: self.tokens_of(files, &s.keywords)?,
            qualifiers: files.words(&s.qualifiers)?,
            modes: files.words(&s.modes)?,
            punctuation: self.tokens_of(files, &s.punctuation)?,
            member: token(&s.member)?,
            call: token(&s.call)?,
            binds: token(&s.binds)?,
            borrow: token(&s.borrow)?,
        };
        let t = &written.tree_sitter;
        let tree_sitter = TreeSitter {
            error: self.kind_named("tree_sitter", &t.error)?,
            end: self.tokens_of(files, &t.end)?,
            word: self.token_named("tree_sitter", &t.word)?,
            extras: t.extras.iter().map(|e| self.token_named("tree_sitter", e)).collect::<Read<_>>()?,
        };
        let text_objects = self.captures(&written.text_objects.nodes, &written.text_objects.places)?;
        Ok(Editors { highlights, outline, text_objects, sublime, tree_sitter })
    }

    fn captures(&self, nodes: &[written::NodeCapture], places: &[written::Place]) -> Read<Captures> {
        let nodes = nodes.iter().map(|n| {
            let named = n.nodes.iter().map(|name| self.node_named(name)).collect::<Read<_>>()?;
            Ok((named, n.capture))
        });
        let places = places.iter().map(|p| {
            Ok(Place {
                kinds: p.kinds.iter().map(|k| self.kind_named("places", k)).collect::<Read<_>>()?,
                field: self.field_named("places", &p.field)?,
                inner: p.inner.as_ref().map(|f| self.field_named("places", f)).transpose()?,
                node: self.kind_named("places", &p.node)?,
                capture: p.capture,
            })
        });
        Ok(Captures { nodes: nodes.collect::<Read<_>>()?, places: places.collect::<Read<_>>()? })
    }

    /// A node kind named `name`, or else a token kind.
    fn node_named(&self, name: &Name) -> Read<Node> {
        match self.kind(&name.0) {
            Some(kind) => Ok(Node::Kind(kind)),
            None => self.token_named("captures", name).map(Node::Token),
        }
    }

    /// The name a tree names a node by.
    pub(crate) fn node_name(&self, node: Node) -> &str {
        match node {
            Node::Kind(kind) => self.kind_name(kind),
            Node::Token(token) => self.token_name(token),
        }
    }
}
