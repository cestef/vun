//! Documents composed at run time: an element's child is a type, so a document is a tree of
//! types the compiler flattens rather than a structure walked with a cursor.

use crate::attr::{Attr, Value};
use crate::escape::Escape;
use crate::render::Render;
use crate::tag::{Kind, Tag};

/// An element with its `N` attributes and its child, which it owns, so a document composes as
/// one expression.
#[derive(Clone, Copy, Debug)]
pub struct Element<'a, const N: usize, C> {
    tag: Tag,
    attrs: [Attr<'a>; N],
    child: C,
}

impl Tag {
    /// This element holding `child`.
    #[must_use]
    pub const fn with<C>(self, child: C) -> Element<'static, 0, C> {
        Element { tag: self, attrs: [], child }
    }

    /// This element with `attrs`, holding nothing until [`Element::with`] gives it a child.
    #[must_use]
    pub const fn attrs<const N: usize>(self, attrs: [Attr<'_>; N]) -> Element<'_, N, ()> {
        Element { tag: self, attrs, child: () }
    }
}

impl<'a, const N: usize> Element<'a, N, ()> {
    /// This element holding `child`.
    #[must_use]
    pub fn with<C>(self, child: C) -> Element<'a, N, C> {
        Element { tag: self.tag, attrs: self.attrs, child }
    }
}

impl<const N: usize, C: Render> Render for Element<'_, N, C> {
    fn render(&self, out: &mut String) {
        Open { tag: self.tag, attrs: &self.attrs }.render(out);
        if self.tag.kind() != Kind::Void {
            self.child.render(out);
            Close(self.tag).render(out);
        }
    }
}

/// A start tag and its attributes.
pub(crate) struct Open<'a> {
    pub(crate) tag: Tag,
    pub(crate) attrs: &'a [Attr<'a>],
}

impl Render for Open<'_> {
    fn render(&self, out: &mut String) {
        out.push('<');
        out.push_str(self.tag.name());
        for attr in self.attrs {
            attr.render(out);
        }
        out.push('>');
    }
}

/// An end tag.
pub(crate) struct Close(pub(crate) Tag);

impl Render for Close {
    fn render(&self, out: &mut String) {
        out.push_str("</");
        out.push_str(self.0.name());
        out.push('>');
    }
}

impl Render for Attr<'_> {
    /// A hole renders as the attribute alone: a template fills it, never the run-time renderer.
    fn render(&self, out: &mut String) {
        out.push(' ');
        out.push_str(self.name.name());
        if let Value::Text(value) = self.value {
            out.push_str("=\"");
            Escape::Attribute.write(out, value);
            out.push('"');
        }
    }
}

/// Text content, escaped when written. Plain strings render the same; this names the intent
/// where a type is wanted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Text<'a>(pub &'a str);

impl Render for Text<'_> {
    fn render(&self, out: &mut String) {
        self.0.render(out);
    }
}

/// Markup written through untouched. [`Raw::new`] takes a literal of the program itself;
/// [`Raw::trusted`] moves the obligation to the caller: what goes in must be safe markup, since
/// nothing downstream checks it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Raw<'a>(&'a str);

impl Raw<'static> {
    #[must_use]
    pub const fn new(markup: &'static str) -> Self {
        Self(markup)
    }
}

impl<'a> Raw<'a> {
    #[must_use]
    pub const fn trusted(markup: &'a str) -> Self {
        Self(markup)
    }

    #[must_use]
    pub const fn get(self) -> &'a str {
        self.0
    }
}

impl Render for Raw<'_> {
    fn render(&self, out: &mut String) {
        out.push_str(self.0);
    }
}

/// The document type declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Doctype;

/// How the declaration is written.
pub(crate) const DOCTYPE: &str = "<!doctype html>";

impl Render for Doctype {
    fn render(&self, out: &mut String) {
        out.push_str(DOCTYPE);
    }
}
