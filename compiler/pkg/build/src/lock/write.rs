//! Writing a lockfile through the literal writer: each list one entry per line.

use std::borrow::Cow;
use std::fmt;

use vun_span::{SourceFiles, Span};
use vun_syntax::literal::{Decl, Literal};

use super::{FILE, LAYOUT, Lock, Locked, Pinned, Replaced, Replacing, field};
use crate::manifest::Identity;

/// The doc comment of the lockfile's first declaration.
const WRITTEN_BY: &str = "Written by `vun pkg` from the selected versions; do not edit.";

impl Lock {
    /// Its declarations, located at `span`.
    fn decls(&self, span: Span) -> Vec<Decl<'_>> {
        let at = |name, value| Decl::new(name, value, span);
        let mut decls =
            vec![Decl { doc: vec![WRITTEN_BY], ..at(field::LAYOUT, Literal::Int(LAYOUT.into())) }];
        decls.extend(self.toolchain.map(|t| at(field::TOOLCHAIN, t.literal(span))));
        decls.push(at(field::DEPS, Literal::List(self.entries.iter().map(|e| e.literal(span)).collect())));
        if !self.replaced.is_empty() {
            let replaced = self.replaced.iter().map(|r| r.literal(span)).collect();
            decls.push(at(field::REPLACE, Literal::List(replaced)));
        }
        decls
    }
}

/// The source of the lockfile, which reads back as the same lock.
impl fmt::Display for Lock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // What is written is located nowhere; the writer reads no spans.
        let span = Span::new(SourceFiles::new().add(FILE, String::new()), 0, 0);
        for decl in self.decls(span) {
            f.write_str(&decl.listed().map_err(|_| fmt::Error)?)?;
        }
        Ok(())
    }
}

impl Locked {
    fn literal(&self, span: Span) -> Literal<'_> {
        let at = |name, value| Decl::new(name, value, span);
        let text = |s: String| Literal::String(Cow::Owned(s));
        let id = self.identity.map_or(Literal::Name(field::FOREIGN), |i| i.literal(span));
        let mut fields = vec![
            at(field::NAME, Literal::String(Cow::Borrowed(&self.name))),
            at(field::VERSION, text(self.version.to_string())),
            at(field::ID, id),
            at(field::HASH, text(self.hash.to_string())),
        ];
        fields.extend(self.via.as_deref().map(|via| at(field::VIA, Literal::String(Cow::Borrowed(via)))));
        Literal::Record(fields)
    }
}

impl Pinned {
    fn literal(self, span: Span) -> Literal<'static> {
        let at = |name, value| Decl::new(name, value, span);
        let text = |s: String| Literal::String(Cow::Owned(s));
        Literal::Record(vec![
            at(field::VERSION, text(self.version.to_string())),
            at(field::HASH, text(self.hash.to_string())),
        ])
    }
}

impl Replaced {
    fn literal(&self, span: Span) -> Literal<'_> {
        let at = |name, value| Decl::new(name, value, span);
        let with = match &self.with {
            Replacing::Stored(hash) => at(field::HASH, Literal::String(Cow::Owned(hash.to_string()))),
            Replacing::Path(path) => at(field::PATH, Literal::String(Cow::Borrowed(path))),
            Replacing::Library(path) => at(field::LIBRARY, Literal::String(Cow::Borrowed(path))),
        };
        Literal::Record(vec![
            at(field::NAME, Literal::String(Cow::Borrowed(&self.name))),
            at(field::ID, self.identity.literal(span)),
            with,
        ])
    }
}

impl Identity {
    fn literal(self, span: Span) -> Literal<'static> {
        let at = |name, value| Decl::new(name, value, span);
        Literal::Record(vec![
            at(field::ID, Literal::Int(self.id.into())),
            at(field::MAJOR, Literal::Int(self.major.into())),
        ])
    }
}
