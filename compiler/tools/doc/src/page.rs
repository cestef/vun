//! The pages: one per namespace, laid in one shell baked at compile time, and an index of the
//! namespaces.

use vun_grammar::PATH_SEPARATOR;
use vun_html::{Each, Render, attr, tag, template};

use crate::item::Item;
use crate::links::Links;
use crate::{Docs, Namespace};

pub(crate) const INDEX: &str = "index.html";
pub(crate) const STYLE: &str = "style.css";
/// The extension of a namespace's page.
const PAGE: &str = "html";
/// The title of the index.
const TITLE: &str = "Documentation";

template! {
    static SHELL = {
        doctype
        html [lang = "en"] {
            head {
                meta [charset = "utf-8"]
                meta [name = "viewport", content = "width=device-width, initial-scale=1"]
                title { _ }
                link [rel = "stylesheet", href = STYLE]
            }
            body { nav { _ } main { _ } }
        }
    }
}

/// The page of the namespace at `path`.
pub(crate) struct Page<'a> {
    pub(crate) docs: &'a Docs,
    pub(crate) links: &'a Links,
    pub(crate) path: &'a [String],
}

impl Page<'_> {
    /// The file of the namespace at `path`, relative to the site: `std.os.html`.
    pub(crate) fn file(path: &[String]) -> String {
        [path.join(PATH_SEPARATOR).as_str(), PAGE].join(".")
    }

    pub(crate) fn html(&self) -> String {
        let namespace = &self.docs.namespaces[self.path];
        let title = self.path.join(PATH_SEPARATOR);
        let nav = (
            tag::a.attrs([attr::href.is(INDEX)]).with(TITLE),
            tag::ul.with(Each(namespace.items.iter().map(Entry))),
        );
        let children = self.docs.children(self.path);
        let within: Vec<&str> = self.path.iter().map(String::as_str).collect();
        let main = (
            tag::h1.with(Crumbs(self.path)),
            self.links.markdown(&within, &namespace.doc),
            (!children.is_empty())
                .then(|| (tag::h2.with("Namespaces"), Listing { namespaces: &children, links: self.links })),
            Each(namespace.items.iter().map(|item| Section { item, outer: Vec::new(), page: self })),
        );
        SHELL.html(&(title.as_str(), nav, main))
    }
}

/// A namespace's path, each prefix a link to its page.
struct Crumbs<'a>(&'a [String]);

impl Render for Crumbs<'_> {
    fn render(&self, out: &mut String) {
        for depth in 1..=self.0.len() {
            if depth > 1 {
                PATH_SEPARATOR.render(out);
            }
            let href = Page::file(&self.0[..depth]);
            tag::a.attrs([attr::href.is(&href)]).with(self.0[depth - 1].as_str()).render(out);
        }
    }
}

/// A declaration in the navigation: a link to its section.
struct Entry<'a>(&'a Item);

impl Render for Entry<'_> {
    fn render(&self, out: &mut String) {
        let href = format!("#{}", self.0.name);
        tag::li.with(tag::a.attrs([attr::href.is(&href)]).with(self.0.name.as_str())).render(out);
    }
}

/// Namespaces, each a link with its summary.
struct Listing<'a> {
    namespaces: &'a [(&'a [String], &'a Namespace)],
    links: &'a Links,
}

impl Render for Listing<'_> {
    fn render(&self, out: &mut String) {
        let listed =
            self.namespaces.iter().map(|&(path, namespace)| Listed { path, namespace, links: self.links });
        tag::ul.attrs([attr::class.is("namespaces")]).with(Each(listed)).render(out);
    }
}

/// A namespace in a listing: its path, a link, and its summary.
struct Listed<'a> {
    path: &'a [String],
    namespace: &'a Namespace,
    links: &'a Links,
}

impl Render for Listed<'_> {
    fn render(&self, out: &mut String) {
        let (href, name) = (Page::file(self.path), self.path.join(PATH_SEPARATOR));
        let link = tag::a.attrs([attr::href.is(&href)]).with(tag::code.with(name.as_str()));
        let within: Vec<&str> = self.path.iter().map(String::as_str).collect();
        let summary = self.links.markdown(&within, &[Item::summary(&self.namespace.doc)]);
        tag::li.with((link, summary)).render(out);
    }
}

/// A declaration's section: its kind and name, its signature, its doc comment and its members.
struct Section<'a> {
    item: &'a Item,
    /// The declarations it is a member of, outermost first, none in a namespace.
    outer: Vec<&'a str>,
    page: &'a Page<'a>,
}

impl Render for Section<'_> {
    fn render(&self, out: &mut String) {
        let item = self.item;
        let anchor = Item::anchor(&self.outer, &item.name);
        let href = format!("#{anchor}");
        let heading = (
            tag::span.attrs([attr::class.is("kind")]).with(item.kind.name()),
            " ",
            tag::a.attrs([attr::href.is(&href)]).with(item.name.as_str()),
        );
        let attributes = Each(item.attributes.iter().map(|a| (a.as_str(), "\n")));
        let inner: Vec<&str> = self.outer.iter().copied().chain([item.name.as_str()]).collect();
        let members = Each(item.members.iter().map(|member| Section {
            item: member,
            outer: inner.clone(),
            page: self.page,
        }));
        let within: Vec<&str> =
            self.page.path.iter().map(String::as_str).chain(self.outer.iter().copied()).collect();
        tag::section
            .attrs([attr::id.is(&anchor), attr::class.is(item.kind.name())])
            .with((
                tag::h3.with(heading),
                tag::pre.with(tag::code.with((attributes, item.signature.as_str()))),
                self.page.links.markdown(&within, &item.doc),
                (!item.members.is_empty()).then(|| tag::div.attrs([attr::class.is("members")]).with(members)),
            ))
            .render(out);
    }
}

/// The index: every namespace, with its summary.
pub(crate) struct Index<'a> {
    pub(crate) docs: &'a Docs,
    pub(crate) links: &'a Links,
}

impl Index<'_> {
    pub(crate) fn html(&self) -> String {
        let all: Vec<_> = self.docs.namespaces.iter().map(|(path, n)| (path.as_slice(), n)).collect();
        SHELL.html(&(TITLE, (), (tag::h1.with(TITLE), Listing { namespaces: &all, links: self.links })))
    }
}
