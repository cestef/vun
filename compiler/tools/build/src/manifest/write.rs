//! Declaring dependencies in a manifest: the `.deps` entry as a literal, inserted by an edit.

use std::borrow::Cow;

use overt_diag::Edit;
use overt_syntax::literal::{Decl, Literal, Unwritable};

use super::{DEP_KINDS, Dep, DepKind, PACKAGE, Source, field};

impl Dep {
    /// Its entry in `.deps`: the fields the reader takes back, without defaults.
    #[must_use]
    pub fn literal(&self) -> Literal<'_> {
        let at = |name, value| Decl { name, doc: Vec::new(), value, span: self.span };
        let text = |s: String| Literal::String(Cow::Owned(s));
        let mut fields = match &self.source {
            Source::Path(path) => vec![at(field::PATH, Literal::String(Cow::Borrowed(path)))],
            Source::Archive { version, hash, urls } => {
                let mut fields = vec![
                    at(field::VERSION, text(version.to_string())),
                    at(field::HASH, text(hash.to_string())),
                ];
                if !urls.is_empty() {
                    let urls = urls.iter().map(|u| Literal::String(Cow::Borrowed(u))).collect();
                    fields.push(at(field::URLS, Literal::List(urls)));
                }
                fields
            }
        };
        let kind = DEP_KINDS.iter().find(|&&(_, k)| k == self.kind && k != DepKind::Overt);
        fields.extend(kind.map(|&(name, _)| at(field::KIND, Literal::Name(name))));
        Literal::Record(fields)
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
            let decl = Decl { name: &self.name, doc: Vec::new(), value: entry, span: self.span };
            package.insert(source, field::DEPS, &Literal::Record(vec![decl]))
        }
    }
}
