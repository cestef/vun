//! The values every method shares: documents and where in them, text edits, and Markdown.

use vun_data::{FromData, ToData};

pub use vun_uri::Uri;

/// A position: a line, and a character in it counted in the encoding agreed at initialization.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, ToData, FromData)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ToData, FromData)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct Location {
    pub uri: Uri,
    pub range: Range,
}

/// How the characters of a line are counted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ToData, FromData)]
pub enum PositionEncoding {
    #[data(name = "utf-8")]
    Utf8,
    #[data(name = "utf-16")]
    Utf16,
    #[data(name = "utf-32")]
    Utf32,
}

impl PositionEncoding {
    /// The encoding when the client names none: the protocol's.
    pub const DEFAULT: Self = Self::Utf16;
}

#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct TextDocumentIdentifier {
    pub uri: Uri,
}

#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct VersionedTextDocumentIdentifier {
    pub uri: Uri,
    pub version: i64,
}

/// A document as the client opens it.
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
#[data(case = camel)]
pub struct TextDocumentItem {
    pub uri: Uri,
    pub language_id: String,
    pub version: i64,
    pub text: String,
}

/// A position in a document, which most requests are about.
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
#[data(case = camel)]
pub struct TextDocumentPositionParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
}

/// The parameters of a request about a whole document.
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
#[data(case = camel)]
pub struct TextDocumentParams {
    pub text_document: TextDocumentIdentifier,
}

/// `range` replaced by `new_text`.
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
#[data(case = camel)]
pub struct TextEdit {
    pub range: Range,
    pub new_text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ToData, FromData)]
pub enum MarkupKind {
    #[data(name = "plaintext")]
    PlainText,
    Markdown,
}

#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct MarkupContent {
    pub kind: MarkupKind,
    pub value: String,
}
