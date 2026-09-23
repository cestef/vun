//! The `.deps` and `.replace` entries of a manifest: where each package comes from.

use overt_diag::{DiagCx, Diagnostic, Msg, errors};
use overt_syntax::literal::{Decl, Literal};

use super::record::{Fields, choice, hash, int, string, strings};
use super::{DEP_KINDS, Dep, DepKind, Identity, Replacement, Source, Version, field};

pub(super) fn deps(decl: &Decl<'_>, cx: &mut DiagCx<'_>) -> Option<Vec<Dep>> {
    let Literal::Record(entries) = &decl.value else {
        return Fields::new(decl, &[], cx).map(|_| Vec::new());
    };
    let deps: Vec<_> = entries.iter().map(|entry| dep(entry, cx)).collect();
    deps.into_iter().collect()
}

fn dep(decl: &Decl<'_>, cx: &mut DiagCx<'_>) -> Option<Dep> {
    let mut fields = Fields::new(decl, field::DEP, cx)?;
    let kind = fields.get(field::KIND).map_or(Some(DepKind::Overt), |d| choice(d, DEP_KINDS, cx));
    let source = source(&mut fields, decl, cx);
    fields.finish(cx);
    Some(Dep { name: decl.name.to_owned(), source: source?, kind: kind?, span: decl.span })
}

/// The `.replace` entries of a manifest.
pub(super) fn replacements(decl: &Decl<'_>, cx: &mut DiagCx<'_>) -> Option<Vec<Replacement>> {
    let Literal::Record(entries) = &decl.value else {
        return Fields::new(decl, &[], cx).map(|_| Vec::new());
    };
    let read: Vec<_> = entries.iter().map(|entry| replacement(entry, cx)).collect();
    read.into_iter().collect()
}

fn replacement(decl: &Decl<'_>, cx: &mut DiagCx<'_>) -> Option<Replacement> {
    let mut fields = Fields::new(decl, field::REPLACEMENT, cx)?;
    let id = fields.require(field::ID).and_then(|d| int(d, cx));
    let major = fields.require(field::MAJOR).and_then(|d| int(d, cx));
    let source = source(&mut fields, decl, cx);
    fields.finish(cx);
    let identity = Identity { id: id?, major: major? };
    Some(Replacement { name: decl.name.to_owned(), identity, source: source?, span: decl.span })
}

/// Where a dependency or a replacement comes from: exactly one of `.path` and `.version` (with
/// `.hash` and optional `.urls`).
fn source(fields: &mut Fields<'_, '_>, decl: &Decl<'_>, cx: &mut DiagCx<'_>) -> Option<Source> {
    match (fields.get(field::PATH), fields.get(field::VERSION)) {
        (Some(path), None) => string(path, cx).map(Source::Path),
        (None, Some(version)) => {
            let version = Version::read(version, cx);
            let hash = fields.require(field::HASH).and_then(|d| hash(d, cx));
            let urls = fields.get(field::URLS).map_or(Some(Vec::new()), |d| strings(d, cx));
            match (version, hash, urls) {
                (Some(version), Some(hash), Some(urls)) => Some(Source::Archive { version, hash, urls }),
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
