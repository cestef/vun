//! JSON-RPC 2.0 as the protocol exchanges it: requests, answered by responses, and notifications,
//! answered by nothing.

use vun_json::{FromJson, ToJson, Value};

/// The version every message names.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ToJson, FromJson)]
pub enum Version {
    #[default]
    #[json(name = "2.0")]
    V2,
}

/// A request's id, as its sender chose it; `null` in the response to a message whose id could
/// not be read.
#[derive(Clone, Debug, PartialEq, Eq, ToJson, FromJson)]
#[json(untagged)]
pub enum Id {
    Number(i64),
    Text(String),
    Null,
}

/// Any message, as the wire holds it: which kind it is follows from the members present.
#[derive(Clone, Debug, PartialEq, Eq, ToJson, FromJson)]
pub struct Message {
    pub jsonrpc: Version,
    pub id: Option<Id>,
    pub method: Option<String>,
    pub params: Option<Value<'static>>,
    pub result: Option<Value<'static>>,
    pub error: Option<ResponseError>,
}

impl Message {
    /// A request of `id` calling `method`.
    #[must_use]
    pub fn request(id: Id, method: &str, params: Value<'static>) -> Self {
        Self { id: Some(id), ..Self::notification(method, params) }
    }

    /// A notification of `method`.
    #[must_use]
    pub fn notification(method: &str, params: Value<'static>) -> Self {
        let method = Some(method.to_owned());
        Self { jsonrpc: Version::V2, id: None, method, params: Some(params), result: None, error: None }
    }

    /// The response to the request `id`: its result, or why it failed.
    #[must_use]
    pub fn response(id: Id, answer: Result<Value<'static>, ResponseError>) -> Self {
        let (result, error) = match answer {
            Ok(result) => (Some(result), None),
            Err(error) => (None, Some(error)),
        };
        Self { jsonrpc: Version::V2, id: Some(id), method: None, params: None, result, error }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, ToJson, FromJson)]
pub struct ResponseError {
    pub code: ErrorCode,
    pub message: String,
}

/// The codes of failures: JSON-RPC's, then the protocol's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ToJson, FromJson)]
#[json(number)]
pub enum ErrorCode {
    ParseError = -32700,
    InvalidRequest = -32600,
    MethodNotFound = -32601,
    InvalidParams = -32602,
    InternalError = -32603,
    ServerNotInitialized = -32002,
    RequestCancelled = -32800,
    ContentModified = -32801,
}
