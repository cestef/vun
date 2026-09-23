//! Reading records and typed values out of literals, reporting where they do not fit.

use overt_diag::{ChildKind, DiagCx, Diagnostic, Msg, errors};
use overt_hash::{Algorithm, Hash};
use overt_suggest::{Lookup, closest};
use overt_syntax::literal::{Decl, Literal};

/// The fields of a record literal, checked against the names a reader knows.
pub(super) struct Fields<'a, 's> {
    record: &'a Decl<'s>,
    decls: &'a [Decl<'s>],
    known: &'static [&'static str],
    missing: Vec<&'static str>,
}

impl<'a, 's> Fields<'a, 's> {
    /// The fields of `record`, which must be a record literal; an empty `.{}` has no fields.
    pub(super) fn new(
        record: &'a Decl<'s>,
        known: &'static [&'static str],
        cx: &mut DiagCx<'_>,
    ) -> Option<Self> {
        let decls: &[Decl<'s>] = match &record.value {
            Literal::Record(decls) => decls,
            Literal::List(items) if items.is_empty() => &[],
            _ => return invalid(record, Msg::new().text("a record ").code(".{ .field = value }"), cx),
        };
        Some(Self { record, decls, known, missing: Vec::new() })
    }

    pub(super) fn get(&self, name: &str) -> Option<&'a Decl<'s>> {
        debug_assert!(self.known.contains(&name), "{name} is declared in the reader's field list");
        self.decls.iter().find(|d| d.name == name)
    }

    /// The field `name`; its absence is reported by [`Self::finish`].
    pub(super) fn require(&mut self, name: &'static str) -> Option<&'a Decl<'s>> {
        let found = self.get(name);
        if found.is_none() {
            self.missing.push(name);
        }
        found
    }

    /// Reports the required fields that are missing, in one diagnostic at the record, and every
    /// field the reader does not know, with the closest known name.
    pub(super) fn finish(self, cx: &mut DiagCx<'_>) {
        if !self.missing.is_empty() {
            let names = self.missing.iter().map(|n| format!(".{n}"));
            let msg = Msg::new().code(self.record.name).text(" is missing ").code_list(names);
            cx.error_at(self.record.span, Diagnostic::error(errors::MANIFEST_MISSING_FIELD, msg));
        }
        for d in self.decls.iter().filter(|d| !self.known.contains(&d.name)) {
            let msg = Msg::new()
                .text("unknown field ")
                .code(format!(".{}", d.name))
                .text(" in ")
                .code(self.record.name);
            let mut diag = Diagnostic::error(errors::MANIFEST_UNKNOWN_FIELD, msg);
            if let Some(near) = closest(d.name, self.known.iter().copied()) {
                diag = diag.child(ChildKind::Help, Msg::did_you_mean(format!(".{near}")));
            }
            cx.error_at(d.span, diag);
        }
    }
}

/// Reports that `decl` is not `expected`.
pub(super) fn invalid<T>(decl: &Decl<'_>, expected: Msg, cx: &mut DiagCx<'_>) -> Option<T> {
    invalid_with(decl, expected, None, cx)
}

/// Reports that `decl` is not `expected`, with a help.
fn invalid_with<T>(decl: &Decl<'_>, expected: Msg, help: Option<Msg>, cx: &mut DiagCx<'_>) -> Option<T> {
    let msg = Msg::new().code(format!(".{}", decl.name)).text(" must be ").append(expected);
    let d = Diagnostic::error(errors::MANIFEST_INVALID_FIELD, msg);
    cx.error_at(decl.span, help.into_iter().fold(d, |d, h| d.child(ChildKind::Help, h)));
    None
}

pub(super) fn string(decl: &Decl<'_>, cx: &mut DiagCx<'_>) -> Option<String> {
    match &decl.value {
        Literal::String(s) => Some(s.to_string()),
        _ => invalid(decl, Msg::new().text("a string"), cx),
    }
}

/// A content hash in its text form, `algorithm:digest`.
pub(super) fn hash(decl: &Decl<'_>, cx: &mut DiagCx<'_>) -> Option<Hash> {
    match &decl.value {
        Literal::String(text) => Hash::parse(text),
        _ => None,
    }
    .or_else(|| {
        let algorithms = Algorithm::ALL.iter().map(|a| format!("{}{}...", a.name(), Hash::SEPARATOR));
        invalid(decl, Msg::new().text("a content hash: ").code_list(algorithms), cx)
    })
}

/// An integer that fits `T`.
pub(super) fn int<T: TryFrom<i128>>(decl: &Decl<'_>, cx: &mut DiagCx<'_>) -> Option<T> {
    match decl.value {
        Literal::Int(i) => {
            T::try_from(i).ok().or_else(|| invalid(decl, Msg::new().text("a smaller integer"), cx))
        }
        _ => invalid(decl, Msg::new().text("an integer"), cx),
    }
}

/// A list of strings: `.{ "a", "b" }` or `[ "a", "b" ]`.
pub(super) fn strings(decl: &Decl<'_>, cx: &mut DiagCx<'_>) -> Option<Vec<String>> {
    let items: &[Literal<'_>] = match &decl.value {
        Literal::List(items) => items,
        Literal::Record(fields) if fields.is_empty() => &[],
        _ => return invalid(decl, Msg::new().text("a list of strings"), cx),
    };
    let texts: Option<Vec<String>> = items.iter().map(|i| i.as_str().map(str::to_owned)).collect();
    texts.or_else(|| invalid(decl, Msg::new().text("a list of strings"), cx))
}

/// One of the names in `table`, written `.name`.
pub(super) fn choice<T: Copy>(decl: &Decl<'_>, table: &[(&str, T)], cx: &mut DiagCx<'_>) -> Option<T> {
    match table.lookup(decl.value.as_name().unwrap_or_default(), |(name, _)| name) {
        Ok(&(_, value)) => Some(value),
        Err(near) => {
            let names = table.iter().map(|(name, _)| format!(".{name}"));
            let help = near.map(|n| Msg::did_you_mean(format!(".{n}")));
            invalid_with(decl, Msg::new().text("one of ").code_list(names), help, cx)
        }
    }
}
