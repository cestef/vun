//! Documents the client holds open, each version read as a new version of its file, and the
//! diagnostics of checking them, published per version.

use std::fmt::Write;

use vun_diag::{Diagnostic, Severity, errors};
use vun_json::{Object, Value};
use vun_origin::Resolve;
use vun_span::SourceMap;
use vun_span::Span;

use crate::message::{Code, Failure, Outgoing};
use crate::position::Lines;
use crate::server::{Document, Server};
use crate::uri::Uri;

/// How diagnostics name where they come from.
const SOURCE: &str = "vun";

impl Server<'_> {
    pub(crate) fn opened(&mut self, params: &Value<'_>) -> Result<Value<'static>, Failure> {
        let item = params.get("textDocument").ok_or(Failure::invalid("textDocument"))?;
        let uri =
            Uri::new(item.get("uri").and_then(Value::as_str).ok_or(Failure::invalid("textDocument.uri"))?);
        let text = item.get("text").and_then(Value::as_str).ok_or(Failure::invalid("textDocument.text"))?;
        let version =
            item.get("version").and_then(Value::as_i64).ok_or(Failure::invalid("textDocument.version"))?;
        self.read(uri, version, text.to_owned())
    }

    pub(crate) fn changed(&mut self, params: &Value<'_>) -> Result<Value<'static>, Failure> {
        let (uri, _) = self.document(params)?;
        let uri = uri.clone();
        let version = params.pointer("/textDocument/version").and_then(Value::as_i64);
        let version = version.ok_or(Failure::invalid("textDocument.version"))?;
        // Every change is the whole text, as the server asks: the last is the document.
        let changes = params.get("contentChanges").map(Value::items);
        let text = changes.and_then(|mut c| c.next_back()?.get("text")?.as_str());
        self.read(uri, version, text.ok_or(Failure::invalid("contentChanges"))?.to_owned())
    }

    pub(crate) fn closed_document(&mut self, params: &Value<'_>) -> Result<Value<'static>, Failure> {
        let (uri, document) = self.document(params)?;
        let (uri, path) = (uri.clone(), document.path.clone());
        self.documents.remove(&uri);
        self.changed.remove(&uri);
        // The file is the one on disk again, and so are its diagnostics once checked.
        self.session.load(&path);
        let params =
            Object::new().with("uri", uri.as_str().to_owned()).with("diagnostics", Vec::<Value>::new());
        self.outbox.push(Outgoing::Notification { method: PUBLISH, params: params.into() });
        Ok(Value::Null)
    }

    /// Reads `text` as the version `version` of the document `uri`.
    fn read(&mut self, uri: Uri, version: i64, text: String) -> Result<Value<'static>, Failure> {
        let path = uri.path().ok_or(Failure::new(Code::InvalidParams, "the document is not a local file"))?;
        let lines = Lines::of(&text);
        let file = self.session.open(&path, text);
        self.documents.insert(uri.clone(), Document { path, version, file, lines });
        self.changed.insert(uri);
        Ok(Value::Null)
    }

    /// Checks each document changed since it was last, and publishes what that reports in it.
    pub(crate) fn publish(&mut self) {
        for uri in std::mem::take(&mut self.changed) {
            let Some(document) = self.documents.get(&uri) else { continue };
            let reported = self.session.reported(document.file);
            let diagnostics: Vec<Value> = reported
                .sorted(&*self.session)
                .into_iter()
                .filter_map(|d| self.diagnostic(d, document))
                .collect();
            let params = Object::new()
                .with("uri", uri.as_str().to_owned())
                .with("version", document.version)
                .with("diagnostics", diagnostics);
            self.outbox.push(Outgoing::Notification { method: PUBLISH, params: params.into() });
        }
    }

    /// `d` as the protocol shows it in `document`, if it is shown there (spec 10, section 6).
    fn diagnostic(&self, d: &Diagnostic, document: &Document) -> Option<Value<'static>> {
        let span = self.session.source_span(d.primary.as_ref()?.origin);
        if span.file != document.file {
            return None;
        }
        let text = self.session.text(document.file);
        let mut message = d.message.to_string();
        for child in &d.children {
            let _ = write!(message, "\n{}: {}", child.kind.name(), child.msg);
        }
        let related: Vec<Value> = d
            .labels
            .iter()
            .filter_map(|l| {
                Some(self.related(self.session.source_span(l.origin), l.msg.as_ref()?.to_string()))
            })
            .collect();
        let mut object = Object::new()
            .with("range", document.lines.range(text, span.range.lo, span.range.hi, self.encoding))
            .with("severity", severity(d.severity))
            .with("code", d.id.to_string())
            .with("source", SOURCE)
            .with("message", message);
        if let Some((_, page)) = errors::EXPLANATIONS.iter().find(|(id, _)| *id == d.id) {
            let page = std::path::absolute(self.session.library.join(page)).ok()?;
            object.push("codeDescription", Object::new().with("href", Uri::of(&page).as_str().to_owned()));
        }
        if !related.is_empty() {
            object.push("relatedInformation", related);
        }
        Some(object.into())
    }

    /// The location of `span` with `message`, as related information.
    fn related(&self, span: Span, message: String) -> Value<'static> {
        Object::new().with("location", self.location(span)).with("message", message).into()
    }

    /// The protocol's location of `span`, in whichever file holds it.
    pub(crate) fn location(&self, span: Span) -> Value<'static> {
        let source = self.session.sources.file(span.file);
        let lines = Lines::of(&source.text);
        let path = std::path::absolute(&source.path).unwrap_or_else(|_| source.path.clone().into());
        let range = lines.range(&source.text, span.range.lo, span.range.hi, self.encoding);
        Object::new().with("uri", Uri::of(&path).as_str().to_owned()).with("range", range).into()
    }
}

/// The notification publishing a document's diagnostics.
const PUBLISH: &str = "textDocument/publishDiagnostics";

/// The protocol's number for `severity`.
const fn severity(severity: Severity) -> u8 {
    match severity {
        Severity::Error => 1,
        Severity::Warning => 2,
        Severity::Info => 3,
        Severity::Hint => 4,
    }
}
