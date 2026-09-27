//! Documents as data, which the Rust compiler walks: the same vocabulary as the run-time
//! elements, shaped as an enum a `const fn` can match, since a trait method cannot run in a
//! constant.

use crate::attr::{Attr, Value};
use crate::element::DOCTYPE;
use crate::escape::Escape;
use crate::tag::{Kind, Tag};

#[derive(Clone, Copy, Debug)]
pub enum Node<'a> {
    Doctype,
    Element(Tag, &'a [Attr<'a>], &'a [Node<'a>]),
    /// Text, escaped.
    Text(&'a str),
    /// Markup written as it is.
    Raw(&'a str),
    /// Filled when the template is rendered.
    Hole,
}

/// A document's nodes, in order.
#[derive(Clone, Copy, Debug)]
pub struct Doc<'a>(pub &'a [Node<'a>]);

/// Where a hole is in a baked document, and what its fill is escaped as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cut {
    pub at: usize,
    pub within: Escape,
}

impl Doc<'_> {
    /// How many bytes the document takes, holes closed up.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.measure().len
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// How many holes the document has.
    #[must_use]
    pub const fn holes(&self) -> usize {
        self.measure().holes
    }

    const fn measure(&self) -> Cursor<'static> {
        let mut cursor = Cursor { bytes: &mut [], cuts: &mut [], len: 0, holes: 0 };
        cursor.nodes(self.0);
        cursor
    }
}

/// Writes a document into buffers sized for it, or only counts when they are empty: one walk
/// both measures and bakes, so the two never disagree.
pub(crate) struct Cursor<'b> {
    pub(crate) bytes: &'b mut [u8],
    pub(crate) cuts: &'b mut [Cut],
    pub(crate) len: usize,
    pub(crate) holes: usize,
}

impl Cursor<'_> {
    pub(crate) const fn nodes(&mut self, nodes: &[Node<'_>]) {
        let mut at = 0;
        while at < nodes.len() {
            self.node(&nodes[at]);
            at += 1;
        }
    }

    const fn node(&mut self, node: &Node<'_>) {
        match *node {
            Node::Doctype => self.str(DOCTYPE),
            Node::Element(tag, attrs, children) => {
                self.str("<");
                self.str(tag.name());
                let mut at = 0;
                while at < attrs.len() {
                    self.attr(&attrs[at]);
                    at += 1;
                }
                self.str(">");
                if !matches!(tag.kind(), Kind::Void) {
                    self.nodes(children);
                    self.str("</");
                    self.str(tag.name());
                    self.str(">");
                }
            }
            Node::Text(text) => self.escaped(text, Escape::Text),
            Node::Raw(markup) => self.str(markup),
            Node::Hole => self.hole(Escape::Text),
        }
    }

    const fn attr(&mut self, attr: &Attr<'_>) {
        self.str(" ");
        self.str(attr.name.name());
        match attr.value {
            Value::On => {}
            Value::Text(value) => {
                self.str("=\"");
                self.escaped(value, Escape::Attribute);
                self.str("\"");
            }
            Value::Hole => {
                self.str("=\"");
                self.hole(Escape::Attribute);
                self.str("\"");
            }
        }
    }

    const fn str(&mut self, text: &str) {
        let bytes = text.as_bytes();
        let mut at = 0;
        while at < bytes.len() {
            self.byte(bytes[at]);
            at += 1;
        }
    }

    const fn escaped(&mut self, text: &str, escape: Escape) {
        let bytes = text.as_bytes();
        let mut at = 0;
        while at < bytes.len() {
            match escape.replacement(bytes[at]) {
                Some(with) => self.str(with),
                None => self.byte(bytes[at]),
            }
            at += 1;
        }
    }

    const fn byte(&mut self, byte: u8) {
        if !self.bytes.is_empty() {
            self.bytes[self.len] = byte;
        }
        self.len += 1;
    }

    const fn hole(&mut self, within: Escape) {
        if !self.cuts.is_empty() {
            self.cuts[self.holes] = Cut { at: self.len, within };
        }
        self.holes += 1;
    }
}
