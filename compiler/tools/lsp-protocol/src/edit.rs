//! Edits across documents: renaming a declaration where it is named, and the fixes of
//! diagnostics as code actions.

use vun_data::{FromData, ToData};

use crate::base::{Range, TextDocumentIdentifier, TextDocumentPositionParams, TextEdit, Uri};
use crate::document::Diagnostic;

#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct ReferenceParams {
    #[data(flatten)]
    pub at: TextDocumentPositionParams,
    pub context: ReferenceContext,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ToData, FromData)]
#[data(case = camel)]
pub struct ReferenceContext {
    /// Whether the declaration is among the locations answered.
    pub include_declaration: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
#[data(case = camel)]
pub struct RenameParams {
    #[data(flatten)]
    pub at: TextDocumentPositionParams,
    pub new_name: String,
}

/// Edits of several documents, applied together.
#[derive(Clone, Debug, Default, PartialEq, Eq, ToData, FromData)]
#[data(case = camel)]
pub struct WorkspaceEdit {
    pub document_changes: Vec<TextDocumentEdit>,
}

/// The edits of one document, none overlapping.
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
#[data(case = camel)]
pub struct TextDocumentEdit {
    pub text_document: OptionalVersionedTextDocumentIdentifier,
    pub edits: Vec<TextEdit>,
}

/// A document, and the version of it the edits are for: `None` for a file the client does not
/// hold open, as read from disk.
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct OptionalVersionedTextDocumentIdentifier {
    pub uri: Uri,
    pub version: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
#[data(case = camel)]
pub struct CodeActionParams {
    pub text_document: TextDocumentIdentifier,
    pub range: Range,
    pub context: CodeActionContext,
}

#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct CodeActionContext {
    /// The diagnostics the client shows over the range.
    pub diagnostics: Vec<Diagnostic>,
    /// The kinds the client asks for; any when absent.
    pub only: Option<Vec<CodeActionKind>>,
}

/// A change the client may apply: a diagnostic's fix.
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
#[data(case = camel)]
pub struct CodeAction {
    pub title: String,
    pub kind: Option<CodeActionKind>,
    /// The diagnostics it fixes.
    pub diagnostics: Option<Vec<Diagnostic>>,
    pub is_preferred: Option<bool>,
    pub edit: Option<WorkspaceEdit>,
}

/// What a code action does; the server offers fixes alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ToData, FromData)]
pub enum CodeActionKind {
    #[data(name = "quickfix")]
    QuickFix,
}
