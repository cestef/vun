//! The `.deps` and `.replace` entries of a manifest: where each package comes from.

use vun_diag::{DiagCx, Diagnostic, Msg, errors};
use vun_syntax::literal::{Decl, Literal};

use super::record::{Fields, Record};
use super::{DEP_KINDS, Dep, DepKind, Identity, Replacement, Source, field};

impl Dep {
    /// The entries of `.deps`.
    pub(super) fn list(decl: &Decl<'_>, cx: &mut DiagCx<'_>) -> Option<Vec<Self>> {
        let Literal::Record(entries) = &decl.value else {
            return Fields::new(decl, &[], cx).map(|_| Vec::new());
        };
        let deps: Vec<_> = entries.iter().map(|entry| Self::read(entry, cx)).collect();
        deps.into_iter().collect()
    }

    fn read(decl: &Decl<'_>, cx: &mut DiagCx<'_>) -> Option<Self> {
        let mut fields = Fields::new(decl, field::DEP, cx)?;
        let kind = fields.get(field::KIND).map_or(Some(DepKind::Vun), |d| d.choice(DEP_KINDS, cx));
        let source = Source::read(&mut fields, decl, cx);
        fields.finish(cx);
        Some(Self { name: decl.name.to_owned(), source: source?, kind: kind?, span: decl.span })
    }
}

impl Replacement {
    /// The entries of `.replace`.
    pub(super) fn list(decl: &Decl<'_>, cx: &mut DiagCx<'_>) -> Option<Vec<Self>> {
        let Literal::Record(entries) = &decl.value else {
            return Fields::new(decl, &[], cx).map(|_| Vec::new());
        };
        let read: Vec<_> = entries.iter().map(|entry| Self::read(entry, cx)).collect();
        read.into_iter().collect()
    }

    fn read(decl: &Decl<'_>, cx: &mut DiagCx<'_>) -> Option<Self> {
        let mut fields = Fields::new(decl, field::REPLACEMENT, cx)?;
        let id = fields.require(field::ID).and_then(|d| d.int(cx));
        let major = fields.require(field::MAJOR).and_then(|d| d.int(cx));
        let source = Source::read(&mut fields, decl, cx);
        fields.finish(cx);
        let identity = Identity { id: id?, major: major? };
        Some(Self { name: decl.name.to_owned(), identity, source: source?, span: decl.span })
    }
}

impl Source {
    /// Where a dependency or a replacement comes from: exactly one of `.path` and `.version`
    /// (with `.hash` and optional `.urls`).
    fn read(fields: &mut Fields<'_, '_>, decl: &Decl<'_>, cx: &mut DiagCx<'_>) -> Option<Self> {
        match (fields.get(field::PATH), fields.get(field::VERSION)) {
            (Some(path), None) => path.string(cx).map(Self::Path),
            (None, Some(version)) => {
                let version = version.version(cx);
                let hash = fields.require(field::HASH).and_then(|d| d.content_hash(cx));
                let urls = fields.get(field::URLS).map_or(Some(Vec::new()), |d| d.strings(cx));
                match (version, hash, urls) {
                    (Some(version), Some(hash), Some(urls)) => Some(Self::Archive { version, hash, urls }),
                    _ => None,
                }
            }
            _ => {
                let msg = Msg::new().code(decl.name).text(" needs exactly one of ");
                let msg =
                    msg.code(format!(".{}", field::PATH)).text(" and ").code(format!(".{}", field::VERSION));
                cx.error_at(decl.span, Diagnostic::error(errors::MANIFEST_INVALID_FIELD, msg));
                None
            }
        }
    }
}
