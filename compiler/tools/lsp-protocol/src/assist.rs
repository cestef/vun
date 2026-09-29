//! What the server offers while code is written: the names that may be written, the signature of
//! the call being written, and the types the code does not write.

use vun_data::{FromData, ToData};

use crate::base::{MarkupContent, Position, Range, TextDocumentIdentifier};

#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
#[data(case = camel)]
pub struct SignatureHelp {
    pub signatures: Vec<SignatureInformation>,
    pub active_signature: Option<u32>,
    pub active_parameter: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct SignatureInformation {
    pub label: String,
    pub documentation: Option<MarkupContent>,
    pub parameters: Option<Vec<ParameterInformation>>,
}

/// A parameter, by its text, which the signature's label holds.
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct ParameterInformation {
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
#[data(case = camel)]
pub struct SignatureHelpOptions {
    /// What typed asks for the signature: opening a call and separating its arguments.
    pub trigger_characters: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
#[data(case = camel)]
pub struct InlayHintParams {
    pub text_document: TextDocumentIdentifier,
    pub range: Range,
}

/// Text shown inline where the source writes none.
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
#[data(case = camel)]
pub struct InlayHint {
    pub position: Position,
    pub label: String,
    pub kind: Option<InlayHintKind>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ToData, FromData)]
#[data(number)]
pub enum InlayHintKind {
    Type = 1,
    Parameter = 2,
}

/// A name that may be written at the position asked about.
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct CompletionItem {
    pub label: String,
    pub kind: Option<CompletionItemKind>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ToData, FromData)]
#[data(number)]
pub enum CompletionItemKind {
    Text = 1,
    Method = 2,
    Function = 3,
    Constructor = 4,
    Field = 5,
    Variable = 6,
    Class = 7,
    Interface = 8,
    Module = 9,
    Property = 10,
    Unit = 11,
    Value = 12,
    Enum = 13,
    Keyword = 14,
    Snippet = 15,
    Color = 16,
    File = 17,
    Reference = 18,
    Folder = 19,
    EnumMember = 20,
    Constant = 21,
    Struct = 22,
    Event = 23,
    Operator = 24,
    TypeParameter = 25,
}

#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
#[data(case = camel)]
pub struct CompletionOptions {
    /// What typed asks for completions besides a name's characters: a member's `.`.
    pub trigger_characters: Vec<String>,
}
