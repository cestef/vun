//! Commands: what a code action or a code lens asks the server to carry out, the lenses showing
//! them over a document, and the documents the server asks the client to show.

use vun_data::{FromData, ToData, Value};

use crate::base::{Range, TextDocumentIdentifier, Uri};

/// A command the server carries out when the client executes it, by name, with its arguments.
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct Command {
    pub title: String,
    pub command: String,
    pub arguments: Option<Vec<Value<'static>>>,
}

#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct ExecuteCommandParams {
    pub command: String,
    pub arguments: Option<Vec<Value<'static>>>,
}

#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct ExecuteCommandOptions {
    /// The names of the commands the server carries out.
    pub commands: Vec<String>,
}

/// A command shown over a range of a document.
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct CodeLens {
    pub range: Range,
    pub command: Option<Command>,
}

#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
#[data(case = camel)]
pub struct CodeLensParams {
    pub text_document: TextDocumentIdentifier,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ToData, FromData)]
#[data(case = camel)]
pub struct CodeLensOptions {
    /// Whether lenses are answered without their commands, each resolved when shown.
    pub resolve_provider: bool,
}

/// A document the server asks the client to show.
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
#[data(case = camel)]
pub struct ShowDocumentParams {
    pub uri: Uri,
    /// Whether it is shown by the system rather than the editor.
    pub external: Option<bool>,
    pub take_focus: Option<bool>,
    pub selection: Option<Range>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ToData, FromData)]
pub struct ShowDocumentResult {
    pub success: bool,
}
