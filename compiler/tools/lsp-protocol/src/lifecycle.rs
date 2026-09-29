//! Initialization: what the client offers, and what the server announces it does.

use vun_json::{FromJson, ToJson, Value};

use crate::base::PositionEncoding;
use crate::language::SemanticTokensLegend;

#[derive(Clone, Debug, Default, PartialEq, Eq, ToJson, FromJson)]
#[json(case = camel)]
pub struct InitializeParams {
    pub process_id: Option<i64>,
    pub root_uri: Option<vun_uri::Uri>,
    pub capabilities: ClientCapabilities,
    pub initialization_options: Option<Value<'static>>,
}

/// What the client can do, as far as the server reads it.
#[derive(Clone, Debug, Default, PartialEq, Eq, ToJson, FromJson)]
#[json(case = camel)]
pub struct ClientCapabilities {
    pub general: Option<GeneralCapabilities>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, ToJson, FromJson)]
#[json(case = camel)]
pub struct GeneralCapabilities {
    pub position_encodings: Option<Vec<Offered>>,
}

/// An encoding the client offers: one the protocol names, or another the server skips.
#[derive(Clone, Debug, PartialEq, Eq, ToJson, FromJson)]
#[json(untagged)]
pub enum Offered {
    Known(PositionEncoding),
    Other(String),
}

impl InitializeParams {
    /// The encodings the client offers that the protocol names, in its order of preference.
    #[must_use]
    pub fn encodings(&self) -> Vec<PositionEncoding> {
        let offered = self.capabilities.general.as_ref().and_then(|g| g.position_encodings.as_ref());
        let known = offered.into_iter().flatten().filter_map(|o| match o {
            Offered::Known(e) => Some(*e),
            Offered::Other(_) => None,
        });
        known.collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, ToJson, FromJson)]
#[json(case = camel)]
pub struct InitializeResult {
    pub capabilities: ServerCapabilities,
    pub server_info: Option<ServerInfo>,
}

#[derive(Clone, Debug, PartialEq, Eq, ToJson, FromJson)]
pub struct ServerInfo {
    pub name: String,
    pub version: Option<String>,
}

/// What the server does; each capability left out is one it lacks.
#[derive(Clone, Debug, Default, PartialEq, Eq, ToJson, FromJson)]
#[json(case = camel)]
pub struct ServerCapabilities {
    pub position_encoding: Option<PositionEncoding>,
    pub text_document_sync: Option<TextDocumentSyncOptions>,
    pub hover_provider: Option<bool>,
    pub definition_provider: Option<bool>,
    pub document_symbol_provider: Option<bool>,
    pub document_formatting_provider: Option<bool>,
    pub folding_range_provider: Option<bool>,
    pub semantic_tokens_provider: Option<SemanticTokensOptions>,
}

#[derive(Clone, Debug, PartialEq, Eq, ToJson, FromJson)]
#[json(case = camel)]
pub struct TextDocumentSyncOptions {
    pub open_close: bool,
    pub change: TextDocumentSyncKind,
}

/// How the client sends a document's changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ToJson, FromJson)]
#[json(number)]
pub enum TextDocumentSyncKind {
    None = 0,
    /// Each change is the document's whole text.
    Full = 1,
    Incremental = 2,
}

#[derive(Clone, Debug, PartialEq, Eq, ToJson, FromJson)]
#[json(case = camel)]
pub struct SemanticTokensOptions {
    pub legend: SemanticTokensLegend,
    pub full: bool,
    pub range: bool,
}
