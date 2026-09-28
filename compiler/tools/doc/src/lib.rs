//! API documentation (spec 12, section 7): the public declarations of packages, read from their
//! concrete syntax trees with their doc comments, as static HTML pages, one per namespace, and
//! as JSON for the language server and indexes. A declaration is public unless its name starts
//! with `_`; a namespace's summary is the first paragraph of its `//!` comment. A code span in a
//! doc comment that names a declaration links to it, by its path or relative to the namespace.

mod item;
mod json;
mod links;
mod page;
mod read;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use vun_diag::{DiagCtxt, DiagCx};
use vun_origin::OriginTable;
use vun_span::SourceFiles;
use vun_syntax::cst::Cst;
use vun_syntax::tokens::Tokens;

pub use item::{Item, Kind};
use links::Links;
use page::{Index, Page};
use read::Reader;

/// A namespace's documentation: its file's doc comment and public declarations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Namespace {
    /// The file it is read from, as the caller names it.
    pub file: String,
    pub doc: Vec<String>,
    pub items: Vec<Item>,
}

impl Namespace {
    /// Marks as types the declarations the reader took for values that `is_type` says are types:
    /// a namespace's own, which are the ones its caller can type by name.
    pub fn retype(&mut self, mut is_type: impl FnMut(&str) -> bool) {
        for item in &mut self.items {
            if item.kind == Kind::Value && is_type(&item.name) {
                item.kind = Kind::Type;
            }
        }
    }
}

/// The documentation of every namespace added, by path.
#[derive(Default)]
pub struct Docs {
    pub namespaces: BTreeMap<Vec<String>, Namespace>,
}

/// A file of the documentation site: its path relative to the site's root, and its contents.
pub struct Written {
    pub path: String,
    pub contents: String,
}

/// The stylesheet every page links.
const STYLE: &str = include_str!("style.css");

impl Docs {
    /// Adds the namespace `path`, read from the source `text` of `file`. A file that does not
    /// parse documents what its tree holds.
    pub fn add(&mut self, path: Vec<String>, file: String, text: &str) {
        let source = SourceFiles::new().add(file.clone(), text);
        let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
        let mut cx = DiagCx { diag: &mut diag, origins: &origins };
        let tokens = Tokens::lex(source, text, &mut cx);
        let cst = Cst::parse(&tokens, text, &mut cx);
        let (doc, items) = Reader { tokens: &tokens, cst: &cst, text }.file();
        self.namespaces.insert(path, Namespace { file, doc, items });
    }

    /// The namespaces directly inside the one at `path`.
    fn children(&self, path: &[String]) -> Vec<(&[String], &Namespace)> {
        let inside = self.namespaces.iter().filter(|(p, _)| p.len() == path.len() + 1 && p.starts_with(path));
        inside.map(|(p, n)| (p.as_slice(), n)).collect()
    }

    /// The site: a page per namespace, an index of them, the stylesheet and the JSON.
    #[must_use]
    pub fn site(&self) -> Vec<Written> {
        let links = Links::of(self);
        let pages = self.namespaces.keys().map(|path| Written {
            path: Page::file(path),
            contents: Page { docs: self, links: &links, path }.html(),
        });
        let index =
            Written { path: page::INDEX.to_owned(), contents: Index { docs: self, links: &links }.html() };
        let style = Written { path: page::STYLE.to_owned(), contents: STYLE.to_owned() };
        let json = Written { path: json::FILE.to_owned(), contents: self.json().to_string() };
        pages.chain([index, style, json]).collect()
    }
}
