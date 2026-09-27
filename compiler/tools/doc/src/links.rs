//! Where each namespace and declaration is documented, so that a doc comment's code span naming
//! one links to it, and doc comments rendered from Markdown with those links.

use std::collections::BTreeMap;

use pulldown_cmark::{CowStr, Event, Parser, html};
use vun_grammar::PATH_SEPARATOR;
use vun_html::{Render, attr, tag};

use crate::Docs;
use crate::item::Item;
use crate::page::Page;

/// The page and anchor of every documented path.
pub(crate) struct Links(BTreeMap<String, String>);

impl Links {
    pub(crate) fn of(docs: &Docs) -> Self {
        let mut links = BTreeMap::new();
        for (path, namespace) in &docs.namespaces {
            let file = Page::file(path);
            let joined = path.join(PATH_SEPARATOR);
            for item in &namespace.items {
                Self::item(&mut links, &file, &joined, &[], item);
            }
            links.insert(joined, file);
        }
        Self(links)
    }

    /// Links `item`, a member of the declarations `outer`, and its members.
    fn item(links: &mut BTreeMap<String, String>, file: &str, namespace: &str, outer: &[&str], item: &Item) {
        let anchor = Item::anchor(outer, &item.name);
        let inner: Vec<&str> = outer.iter().copied().chain([item.name.as_str()]).collect();
        for member in &item.members {
            Self::item(links, file, namespace, &inner, member);
        }
        links.insert([namespace, &anchor].join(PATH_SEPARATOR), format!("{file}#{anchor}"));
    }

    /// Where the code `code` written at `within` points: a path as written, or one relative to
    /// `within` (a namespace's path, then the declarations enclosing the doc comment) or to a
    /// scope around it.
    pub(crate) fn resolve(&self, within: &[&str], code: &str) -> Option<&str> {
        let code = code.strip_suffix("()").unwrap_or(code);
        (0..=within.len()).rev().find_map(|depth| {
            let path: Vec<&str> = within[..depth].iter().copied().chain([code]).collect();
            self.0.get(&path.join(PATH_SEPARATOR)).map(String::as_str)
        })
    }

    /// The doc comment `doc`, written at `within`, rendered from Markdown: the renderer escapes
    /// what it writes, and code spans naming a declaration become links.
    pub(crate) fn markdown(&self, within: &[&str], doc: &[String]) -> Markup {
        let text = doc.join("\n");
        let events = Parser::new(&text).map(|event| match event {
            Event::Code(code) => match self.resolve(within, &code) {
                Some(href) => {
                    let link = tag::a.attrs([attr::href.is(href)]).with(tag::code.with(&*code));
                    Event::InlineHtml(CowStr::from(link.html()))
                }
                None => Event::Code(code),
            },
            other => other,
        });
        let mut markup = String::new();
        html::push_html(&mut markup, events);
        Markup(markup)
    }
}

/// Markup the Markdown renderer made, which escapes what it writes: written as it is.
pub(crate) struct Markup(String);

impl Render for Markup {
    fn render(&self, out: &mut String) {
        out.push_str(&self.0);
    }
}
