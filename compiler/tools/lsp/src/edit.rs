//! Edits across documents: where a declaration is named and renaming it there, and the fixes of
//! diagnostics as code actions.

use std::collections::BTreeMap;

use vun_diag::Safety;
use vun_lsp_protocol::base::{Location, Range, TextEdit, Uri};
use vun_lsp_protocol::edit::{
    CodeAction, CodeActionKind, CodeActionParams, OptionalVersionedTextDocumentIdentifier, ReferenceParams,
    RenameParams, TextDocumentEdit, WorkspaceEdit,
};
use vun_lsp_protocol::requests as r;
use vun_lsp_protocol::rpc::ErrorCode;
use vun_origin::Resolve;
use vun_span::Span;

use crate::failure::{Failure, Reason};
use crate::methods::Answers;
use crate::server::Server;

impl Answers<r::References> for Server<'_> {
    fn answer(&mut self, params: ReferenceParams) -> Result<Vec<Location>, Failure> {
        let (file, offset) = self.at(&params.at)?;
        let Some(occurrences) = self.session.references(file, offset) else { return Ok(Vec::new()) };
        let spans = if params.context.include_declaration { occurrences.all() } else { occurrences.uses };
        Ok(spans.into_iter().filter_map(|span| self.location(span)).collect())
    }
}

impl Answers<r::Rename> for Server<'_> {
    fn answer(&mut self, params: RenameParams) -> Result<Option<WorkspaceEdit>, Failure> {
        let (file, offset) = self.at(&params.at)?;
        let renamed = self
            .session
            .rename(file, offset, &params.new_name)
            .map_err(|unrenamed| Failure::new(ErrorCode::RequestFailed, Reason::Rename(unrenamed)))?;
        Ok(Some(self.workspace_edit(renamed.spans.into_iter().map(|span| (span, renamed.text.clone())))))
    }
}

impl Answers<r::CodeAction> for Server<'_> {
    fn answer(&mut self, params: CodeActionParams) -> Result<Vec<CodeAction>, Failure> {
        let asked = params.context.only.as_ref().is_none_or(|only| only.contains(&CodeActionKind::QuickFix));
        if !asked {
            return Ok(Vec::new());
        }
        let file = self.document(&params.text_document)?.file;
        let reported = self.session.reported(file);
        let document = self.document(&params.text_document)?;
        let mut actions = Vec::new();
        for d in reported.sorted(&*self.session) {
            let Some(shown) = self.diagnostic(d, document).filter(|s| overlaps(s.range, params.range)) else {
                continue;
            };
            for fix in d.fixes.iter().filter(|f| f.safety != Safety::DisplayOnly) {
                // Edits of a text the client no longer holds would land elsewhere.
                if fix.edits.iter().any(|e| self.session.current(e.span.file) != e.span.file) {
                    continue;
                }
                let edits = fix.edits.iter().map(|e| (e.span, e.text.clone()));
                actions.push(CodeAction {
                    title: fix.msg.to_string(),
                    kind: Some(CodeActionKind::QuickFix),
                    diagnostics: Some(vec![shown.clone()]),
                    is_preferred: Some(fix.preferred),
                    edit: Some(self.workspace_edit(edits)),
                });
            }
        }
        Ok(actions)
    }
}

impl Server<'_> {
    /// Each span replaced by its text, grouped by document: one the client holds is edited at the
    /// version it sent, any other as on disk.
    fn workspace_edit(&self, edits: impl Iterator<Item = (Span, String)>) -> WorkspaceEdit {
        let mut by_document: BTreeMap<Uri, Vec<TextEdit>> = BTreeMap::new();
        for (span, new_text) in edits {
            let Some(location) = self.location(span) else { continue };
            by_document.entry(location.uri).or_default().push(TextEdit { range: location.range, new_text });
        }
        let document_changes = by_document
            .into_iter()
            .map(|(uri, edits)| {
                let version = self.documents.get(&uri).map(|d| d.version);
                TextDocumentEdit {
                    text_document: OptionalVersionedTextDocumentIdentifier { uri, version },
                    edits,
                }
            })
            .collect();
        WorkspaceEdit { document_changes }
    }
}

/// Whether two ranges share a position, their ends included: a cursor touching a diagnostic
/// asks for its fixes.
fn overlaps(a: Range, b: Range) -> bool {
    a.start <= b.end && b.start <= a.end
}
