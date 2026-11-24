//! Reading records and typed values out of literals, reporting where they do not fit.

use vun_diag::{ChildKind, DiagCx, Diagnostic, Msg, errors};
use vun_hash::{Algorithm, Hash, Snapshot};
use vun_intern::Symbol;
use vun_span::Version;
use vun_suggest::{Lookup, closest};
use vun_syntax::literal::{Decl, Literal};
use vun_uri::Uri;

/// The fields of a record literal, checked against the names a reader knows.
pub(crate) struct Fields<'a, 's> {
    record: &'a Decl<'s>,
    decls: &'a [Decl<'s>],
    known: &'static [&'static str],
    missing: Vec<&'static str>,
}

impl<'a, 's> Fields<'a, 's> {
    /// The fields of `record`, which must be a record literal; an empty `.{}` has no fields.
    pub(crate) fn new(
        record: &'a Decl<'s>,
        known: &'static [&'static str],
        cx: &mut DiagCx<'_>,
    ) -> Option<Self> {
        let decls: &[Decl<'s>] = match &record.value {
            Literal::Record(decls) => decls,
            Literal::List(items) if items.is_empty() => &[],
            _ => return record.invalid(Msg::new().text("a record ").code(".{ .field = value }"), cx),
        };
        Some(Self { record, decls, known, missing: Vec::new() })
    }

    pub(crate) fn get(&self, name: &str) -> Option<&'a Decl<'s>> {
        debug_assert!(self.known.contains(&name), "{name} is declared in the reader's field list");
        self.decls.iter().find(|d| d.name == name)
    }

    /// The field `name`; its absence is reported by [`Self::finish`].
    pub(crate) fn require(&mut self, name: &'static str) -> Option<&'a Decl<'s>> {
        let found = self.get(name);
        if found.is_none() {
            self.missing.push(name);
        }
        found
    }

    /// Reports the required fields that are missing, in one diagnostic at the record, and every
    /// field the reader does not know, with the closest known name.
    pub(crate) fn finish(self, cx: &mut DiagCx<'_>) {
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

/// Reading a declaration's literal as a typed value, reporting when it does not fit.
pub(crate) trait Record {
    /// Reports that this declaration is not `expected`.
    fn invalid<T>(&self, expected: Msg, cx: &mut DiagCx<'_>) -> Option<T>;
    /// Reports that this declaration is not `expected`, with a help.
    fn invalid_with<T>(&self, expected: Msg, help: Option<Msg>, cx: &mut DiagCx<'_>) -> Option<T>;
    fn string(&self, cx: &mut DiagCx<'_>) -> Option<String>;
    /// A content hash in its text form, `algorithm:digest`.
    fn content_hash(&self, cx: &mut DiagCx<'_>) -> Option<Hash>;
    /// A version, `"major.minor.patch"`.
    fn version(&self, cx: &mut DiagCx<'_>) -> Option<Version>;
    /// An integer that fits `T`.
    fn int<T: TryFrom<i128>>(&self, cx: &mut DiagCx<'_>) -> Option<T>;
    /// A list of strings: `.{ "a", "b" }` or `[ "a", "b" ]`.
    fn strings(&self, cx: &mut DiagCx<'_>) -> Option<Vec<String>>;
    /// A list of URLs (RFC 3986).
    fn uris(&self, cx: &mut DiagCx<'_>) -> Option<Vec<Uri>>;
    /// A list of strings, as symbols.
    fn symbols(&self, cx: &mut DiagCx<'_>) -> Option<Vec<Symbol>>;
    /// A list of paths inside a directory, as snapshots name them: no root, no `..`.
    fn paths(&self, cx: &mut DiagCx<'_>) -> Option<Vec<String>>;
    /// One of the names in `table`, written `.name`.
    fn choice<T: Copy>(&self, table: &[(&str, T)], cx: &mut DiagCx<'_>) -> Option<T>;
    /// A list of names in `table`, each written `.name`.
    fn choices<T: Copy>(&self, table: &[(&str, T)], cx: &mut DiagCx<'_>) -> Option<Vec<T>>;
    /// The items of a list, each read by `read` as a record at the list's source.
    fn items<T>(
        &self,
        read: impl Fn(&Decl<'_>, &mut DiagCx<'_>) -> Option<T>,
        cx: &mut DiagCx<'_>,
    ) -> Option<Vec<T>>;
}

impl Record for Decl<'_> {
    fn invalid<T>(&self, expected: Msg, cx: &mut DiagCx<'_>) -> Option<T> {
        self.invalid_with(expected, None, cx)
    }

    fn invalid_with<T>(&self, expected: Msg, help: Option<Msg>, cx: &mut DiagCx<'_>) -> Option<T> {
        let msg = Msg::new().code(format!(".{}", self.name)).text(" must be ").append(expected);
        let d = Diagnostic::error(errors::MANIFEST_INVALID_FIELD, msg);
        cx.error_at(self.span, help.into_iter().fold(d, |d, h| d.child(ChildKind::Help, h)));
        None
    }

    fn string(&self, cx: &mut DiagCx<'_>) -> Option<String> {
        match &self.value {
            Literal::String(s) => Some(s.to_string()),
            _ => self.invalid(Msg::new().text("a string"), cx),
        }
    }

    fn content_hash(&self, cx: &mut DiagCx<'_>) -> Option<Hash> {
        match &self.value {
            Literal::String(text) => text.parse().ok(),
            _ => None,
        }
        .or_else(|| {
            let algorithms = Algorithm::ALL.iter().map(|a| format!("{}{}...", a.name(), Hash::SEPARATOR));
            self.invalid(Msg::new().text("a content hash: ").code_list(algorithms), cx)
        })
    }

    fn version(&self, cx: &mut DiagCx<'_>) -> Option<Version> {
        match &self.value {
            Literal::String(text) => text.parse::<Version>().ok(),
            _ => None,
        }
        .or_else(|| self.invalid(Msg::new().text("a version ").code("\"major.minor.patch\""), cx))
    }

    fn int<T: TryFrom<i128>>(&self, cx: &mut DiagCx<'_>) -> Option<T> {
        match self.value {
            Literal::Int(i) => {
                T::try_from(i).ok().or_else(|| self.invalid(Msg::new().text("a smaller integer"), cx))
            }
            _ => self.invalid(Msg::new().text("an integer"), cx),
        }
    }

    fn strings(&self, cx: &mut DiagCx<'_>) -> Option<Vec<String>> {
        let items: &[Literal<'_>] = match &self.value {
            Literal::List(items) => items,
            Literal::Record(fields) if fields.is_empty() => &[],
            _ => return self.invalid(Msg::new().text("a list of strings"), cx),
        };
        let texts: Option<Vec<String>> = items.iter().map(|i| i.as_str().map(str::to_owned)).collect();
        texts.or_else(|| self.invalid(Msg::new().text("a list of strings"), cx))
    }

    fn uris(&self, cx: &mut DiagCx<'_>) -> Option<Vec<Uri>> {
        let parsed: Result<Vec<Uri>, _> = self.strings(cx)?.into_iter().map(Uri::try_from).collect();
        parsed.map_or_else(
            |invalid| {
                let help = Msg::new().text("each is a URL: ").text(invalid.to_string());
                self.invalid_with(Msg::new().text("a list of URLs"), Some(help), cx)
            },
            Some,
        )
    }

    fn symbols(&self, cx: &mut DiagCx<'_>) -> Option<Vec<Symbol>> {
        self.strings(cx).map(|all| all.iter().map(|s| Symbol::intern(s)).collect())
    }

    fn paths(&self, cx: &mut DiagCx<'_>) -> Option<Vec<String>> {
        let paths = self.strings(cx)?;
        if let Some(outside) = paths.iter().find(|p| Snapshot::components(p).is_none()) {
            let msg = Msg::new().text("a list of paths inside the package, not ").code(outside.as_str());
            return self.invalid(msg, cx);
        }
        Some(paths)
    }

    fn choice<T: Copy>(&self, table: &[(&str, T)], cx: &mut DiagCx<'_>) -> Option<T> {
        match table.lookup(self.value.as_name().unwrap_or_default(), |(name, _)| name) {
            Ok(&(_, value)) => Some(value),
            Err(near) => {
                let names = table.iter().map(|(name, _)| format!(".{name}"));
                let help = near.map(|n| Msg::did_you_mean(format!(".{n}")));
                self.invalid_with(Msg::new().text("one of ").code_list(names), help, cx)
            }
        }
    }

    fn choices<T: Copy>(&self, table: &[(&str, T)], cx: &mut DiagCx<'_>) -> Option<Vec<T>> {
        self.items(|item, cx| item.choice(table, cx), cx)
    }

    fn items<T>(
        &self,
        read: impl Fn(&Decl<'_>, &mut DiagCx<'_>) -> Option<T>,
        cx: &mut DiagCx<'_>,
    ) -> Option<Vec<T>> {
        let items: &[Literal<'_>] = match &self.value {
            Literal::List(items) => items,
            Literal::Record(fields) if fields.is_empty() => &[],
            _ => return self.invalid(Msg::new().text("a list of records"), cx),
        };
        let read: Vec<Option<T>> =
            items.iter().map(|item| read(&Decl::new(self.name, item.clone(), self.span), cx)).collect();
        read.into_iter().collect()
    }
}
