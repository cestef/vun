//! Writing manifests: a whole `package` literal, or a `.deps` entry inserted by an edit, both
//! through the literal writer, so that what is written reads back as the same manifest.

use std::borrow::Cow;

use vun_diag::Edit;
use vun_syntax::literal::{Decl, Literal, Unwritable};

use vun_span::Span;
use vun_uri::Uri;

use super::{DEFAULT_PRELUDE, DEP_KINDS, Dep, DepKind, Manifest, PACKAGE, Replacement, Source, field};

impl Manifest {
    /// Its `package` literal: the fields the reader takes back, without defaults.
    #[must_use]
    pub fn literal(&self) -> Literal<'_> {
        let at = |name, value| Decl::new(name, value, self.prelude_span);
        let text = |s: String| Literal::String(Cow::Owned(s));
        let mut fields = vec![
            at(field::NAME, Literal::String(Cow::Borrowed(&self.name))),
            at(field::ID, Literal::Int(self.id.into())),
            at(field::VERSION, text(self.version.to_string())),
            at(field::EDITION, Literal::Int(self.edition.into())),
        ];
        fields.extend(self.toolchain.map(|v| at(field::TOOLCHAIN, text(v.to_string()))));
        fields.extend(self.target.clone().map(|t| at(field::TARGET, text(t))));
        fields.push(at(field::INCLUDE, Literal::strings(self.include.iter().map(String::as_str))));
        if !self.deps.is_empty() {
            let deps = self.deps.iter().map(|d| at(&d.name, d.literal())).collect();
            fields.push(at(field::DEPS, Literal::Record(deps)));
        }
        if !self.replace.is_empty() {
            let replace = self.replace.iter().map(|r| at(&r.name, r.literal())).collect();
            fields.push(at(field::REPLACE, Literal::Record(replace)));
        }
        if !self.index.is_empty() {
            fields.push(at(field::INDEX, Literal::strings(self.index.iter().map(Uri::as_str))));
        }
        if !self.members.is_empty() {
            fields.push(at(field::MEMBERS, Literal::strings(self.members.iter().map(String::as_str))));
        }
        if self.prelude != [DEFAULT_PRELUDE] {
            fields.push(at(field::PRELUDE, Literal::strings(self.prelude.iter().map(String::as_str))));
        }
        Literal::Record(fields)
    }

    /// The `package` declaration of a `build.vun`.
    ///
    /// # Errors
    /// Returns why a field has no literal source.
    pub fn source(&self) -> Result<String, Unwritable> {
        Decl::new(PACKAGE, self.literal(), self.prelude_span).source()
    }
}

impl Source {
    /// Its fields in a `.deps` or `.replace` entry.
    fn fields(&self, span: Span) -> Vec<Decl<'_>> {
        let at = |name, value| Decl::new(name, value, span);
        let text = |s: String| Literal::String(Cow::Owned(s));
        match self {
            Self::Path(path) => vec![at(field::PATH, Literal::String(Cow::Borrowed(path)))],
            Self::Archive { version, hash, urls } => {
                let mut fields = vec![
                    at(field::VERSION, text(version.to_string())),
                    at(field::HASH, text(hash.to_string())),
                ];
                if !urls.is_empty() {
                    fields.push(at(field::URLS, Literal::strings(urls.iter().map(Uri::as_str))));
                }
                fields
            }
        }
    }
}

impl Replacement {
    /// Its entry in `.replace`.
    #[must_use]
    pub fn literal(&self) -> Literal<'_> {
        let at = |name, value| Decl::new(name, value, self.span);
        let mut fields = vec![
            at(field::ID, Literal::Int(self.identity.id.into())),
            at(field::MAJOR, Literal::Int(self.identity.major.into())),
        ];
        fields.extend(self.source.fields(self.span));
        Literal::Record(fields)
    }
}

impl Dep {
    /// Its entry in `.deps`: the fields the reader takes back, without defaults.
    #[must_use]
    pub fn literal(&self) -> Literal<'_> {
        let mut fields = self.source.fields(self.span);
        let kind = DEP_KINDS.iter().find(|&&(_, k)| k == self.kind && k != DepKind::Vun);
        let at = |name, value| Decl::new(name, value, self.span);
        fields.extend(kind.map(|&(name, _)| at(field::KIND, Literal::Name(name))));
        Literal::Record(fields)
    }

    /// The edit that rewrites its existing entry in `.deps`, found by name, in the manifest whose
    /// declarations `decls` were read from `source`.
    ///
    /// # Errors
    /// Returns why the entry cannot be written, or that there is none.
    pub fn redeclare(&self, decls: &[Decl<'_>], source: &str) -> Result<Edit, Unwritable> {
        let package = decls.iter().find(|d| d.name == PACKAGE).ok_or(Unwritable::NotRecord)?;
        let deps = package.value.decls().and_then(|fields| fields.iter().find(|d| d.name == field::DEPS));
        let entry = deps.and_then(|d| d.value.decls()?.iter().find(|e| e.name == self.name));
        entry.ok_or(Unwritable::NotRecord)?.set(source, &self.literal())
    }

    /// The edit that declares it in the manifest whose declarations `decls` were read from
    /// `source`: into `.deps`, or into a new `.deps` field.
    ///
    /// # Errors
    /// Returns why the entry cannot be written there.
    pub fn declare(&self, decls: &[Decl<'_>], source: &str) -> Result<Edit, Unwritable> {
        let package = decls.iter().find(|d| d.name == PACKAGE).ok_or(Unwritable::NotRecord)?;
        let entry = self.literal();
        if let Some(deps) =
            package.value.decls().and_then(|fields| fields.iter().find(|d| d.name == field::DEPS))
        {
            deps.insert(source, &self.name, &entry)
        } else {
            let decl = Decl::new(&self.name, entry, self.span);
            package.insert(source, field::DEPS, &Literal::Record(vec![decl]))
        }
    }
}
