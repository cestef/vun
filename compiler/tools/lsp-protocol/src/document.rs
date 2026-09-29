//! Documents the client holds open, and the diagnostics published about them.

use vun_json::{FromJson, ToJson};

use crate::base::{
    Location, Range, TextDocumentIdentifier, TextDocumentItem, Uri, VersionedTextDocumentIdentifier,
};

#[derive(Clone, Debug, PartialEq, Eq, ToJson, FromJson)]
#[json(case = camel)]
pub struct DidOpenParams {
    pub text_document: TextDocumentItem,
}

#[derive(Clone, Debug, PartialEq, Eq, ToJson, FromJson)]
#[json(case = camel)]
pub struct DidChangeParams {
    pub text_document: VersionedTextDocumentIdentifier,
    pub content_changes: Vec<ContentChange>,
}

/// A change: the whole text, or, with a range, the text replacing it.
#[derive(Clone, Debug, PartialEq, Eq, ToJson, FromJson)]
pub struct ContentChange {
    pub range: Option<Range>,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, ToJson, FromJson)]
#[json(case = camel)]
pub struct DidCloseParams {
    pub text_document: TextDocumentIdentifier,
}

#[derive(Clone, Debug, PartialEq, Eq, ToJson, FromJson)]
pub struct PublishDiagnosticsParams {
    pub uri: Uri,
    pub version: Option<i64>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, PartialEq, Eq, ToJson, FromJson)]
#[json(case = camel)]
pub struct Diagnostic {
    pub range: Range,
    pub severity: Option<Severity>,
    pub code: Option<String>,
    pub code_description: Option<CodeDescription>,
    pub source: Option<String>,
    pub message: String,
    pub related_information: Option<Vec<RelatedInformation>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ToJson, FromJson)]
#[json(number)]
pub enum Severity {
    Error = 1,
    Warning = 2,
    Information = 3,
    Hint = 4,
}

#[derive(Clone, Debug, PartialEq, Eq, ToJson, FromJson)]
pub struct CodeDescription {
    pub href: Uri,
}

#[derive(Clone, Debug, PartialEq, Eq, ToJson, FromJson)]
pub struct RelatedInformation {
    pub location: Location,
    pub message: String,
}
