//! Documents the client holds open, each version read as a new version of its file, and the
//! diagnostics of checking them, published per version.

use std::fmt::Write;

use vun_diag::{Diagnostic, Severity, errors};
use vun_lsp_protocol::base::{Location, Uri};
use vun_lsp_protocol::document::{
    self as protocol, CodeDescription, ContentChange, DidChangeParams, DidCloseParams, DidOpenParams,
    PublishDiagnosticsParams, RelatedInformation,
};
use vun_lsp_protocol::rpc::{ErrorCode, Message};
use vun_lsp_protocol::{Notification, notifications as n};
use vun_origin::Resolve;
use vun_span::{SourceMap, Span};

use crate::failure::{Failure, Reason};
use crate::methods::Hears;
use crate::position::Lines;
use crate::server::{Document, Server};

/// How diagnostics name where they come from.
const SOURCE: &str = "vun";

impl Hears<n::DidOpen> for Server<'_> {
    fn hear(&mut self, params: DidOpenParams) -> Result<(), Failure> {
        let document = params.text_document;
        self.read(document.uri, document.version, document.text)
    }
}

impl Hears<n::DidChange> for Server<'_> {
    fn hear(&mut self, params: DidChangeParams) -> Result<(), Failure> {
        let identifier = params.text_document;
        let document = self
            .documents
            .get(&identifier.uri)
            .ok_or(Failure::new(ErrorCode::InvalidParams, Reason::Closed))?;
        let mut text = self.session.text(document.file).to_owned();
        for change in &params.content_changes {
            self.apply(&mut text, change);
        }
        self.read(identifier.uri, identifier.version, text)
    }
}

impl Hears<n::DidClose> for Server<'_> {
    fn hear(&mut self, params: DidCloseParams) -> Result<(), Failure> {
        let uri = params.text_document.uri;
        let document =
            self.documents.remove(&uri).ok_or(Failure::new(ErrorCode::InvalidParams, Reason::Closed))?;
        self.changed.remove(&uri);
        // The file is the one on disk again, and so are its diagnostics once checked.
        self.session.load(&document.path);
        self.notify::<n::PublishDiagnostics>(&PublishDiagnosticsParams {
            uri,
            version: None,
            diagnostics: Vec::new(),
        });
        Ok(())
    }
}

impl Server<'_> {
    /// Queues the notification `N` of `params`.
    pub(crate) fn notify<N: Notification>(&mut self, params: &N::Params) {
        self.outbox.push(Message::notification(N::METHOD, vun_json::ToJson::to_json(params)));
    }

    /// Applies `change` to `text`: a range replaced, or the whole text.
    fn apply(&self, text: &mut String, change: &ContentChange) {
        let Some(range) = change.range else {
            text.clone_from(&change.text);
            return;
        };
        let lines = Lines::of(text);
        let end = u32::try_from(text.len()).unwrap_or(u32::MAX);
        let lo = lines.offset(text, range.start, self.encoding).unwrap_or(end) as usize;
        let hi = lines.offset(text, range.end, self.encoding).unwrap_or(end) as usize;
        text.replace_range(lo..hi.max(lo), &change.text);
    }

    /// Reads `text` as the version `version` of the document `uri`.
    fn read(&mut self, uri: Uri, version: i64, text: String) -> Result<(), Failure> {
        let path =
            uri.to_file_path().map_err(|e| Failure::new(ErrorCode::InvalidParams, Reason::NotLocal(e)))?;
        let lines = Lines::of(&text);
        let file = self.session.open(&path, text);
        self.documents.insert(uri.clone(), Document { path, version, file, lines });
        self.changed.insert(uri);
        Ok(())
    }

    /// Checks each document changed since it was last, and publishes what that reports in it.
    pub(crate) fn publish(&mut self) {
        for uri in std::mem::take(&mut self.changed) {
            let Some(document) = self.documents.get(&uri) else { continue };
            let reported = self.session.reported(document.file);
            let diagnostics = reported
                .sorted(&*self.session)
                .into_iter()
                .filter_map(|d| self.diagnostic(d, document))
                .collect();
            let params = PublishDiagnosticsParams { uri, version: Some(document.version), diagnostics };
            self.notify::<n::PublishDiagnostics>(&params);
        }
    }

    /// `d` as the protocol shows it in `document`, if it is shown there (spec 10, section 6).
    fn diagnostic(&self, d: &Diagnostic, document: &Document) -> Option<protocol::Diagnostic> {
        let span = self.session.source_span(d.primary.as_ref()?.origin);
        if span.file != document.file {
            return None;
        }
        let text = self.session.text(document.file);
        let mut message = d.message.to_string();
        for child in &d.children {
            let _ = write!(message, "\n{}: {}", child.kind.name(), child.msg);
        }
        let related: Vec<RelatedInformation> = d
            .labels
            .iter()
            .filter_map(|l| {
                let message = l.msg.as_ref()?.to_string();
                Some(RelatedInformation {
                    location: self.location(self.session.source_span(l.origin))?,
                    message,
                })
            })
            .collect();
        let page = errors::EXPLANATIONS.iter().find(|(id, _)| *id == d.id);
        let page = page.and_then(|(_, page)| std::path::absolute(self.session.library.join(page)).ok());
        Some(protocol::Diagnostic {
            range: document.lines.range(text, span.range.lo, span.range.hi, self.encoding),
            severity: Some(severity(d.severity)),
            code: Some(d.id.to_string()),
            code_description: page
                .and_then(|p| Uri::from_file_path(&p).ok())
                .map(|href| CodeDescription { href }),
            source: Some(SOURCE.to_owned()),
            message,
            related_information: (!related.is_empty()).then_some(related),
        })
    }

    /// The protocol's location of `span`, in whichever file holds it.
    pub(crate) fn location(&self, span: Span) -> Option<Location> {
        let source = self.session.sources.file(span.file);
        let lines = Lines::of(&source.text);
        let path = std::path::absolute(&source.path).ok()?;
        let range = lines.range(&source.text, span.range.lo, span.range.hi, self.encoding);
        Some(Location { uri: Uri::from_file_path(&path).ok()?, range })
    }
}

/// The protocol's severity of `severity`.
const fn severity(severity: Severity) -> protocol::Severity {
    match severity {
        Severity::Error => protocol::Severity::Error,
        Severity::Warning => protocol::Severity::Warning,
        Severity::Info => protocol::Severity::Information,
        Severity::Hint => protocol::Severity::Hint,
    }
}
