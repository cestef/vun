//! JSON-RPC messages as the protocol exchanges them: requests, which a response answers,
//! notifications, which nothing answers, and the errors a response may carry.

use std::fmt;

use vun_json::{Object, Value};

/// The version of JSON-RPC every message names.
const VERSION: &str = "2.0";

/// A request's id, as the client chose it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Id {
    Number(i64),
    Text(String),
}

impl Id {
    fn read(value: &Value<'_>) -> Option<Self> {
        match value {
            Value::String(text) => Some(Self::Text(text.to_string())),
            _ => value.as_i64().map(Self::Number),
        }
    }

    fn value(&self) -> Value<'static> {
        match self {
            Self::Number(n) => Value::from(*n),
            Self::Text(text) => Value::from(text.clone()),
        }
    }
}

/// A message from the client.
pub(crate) enum Incoming {
    Request {
        id: Id,
        method: String,
        params: Value<'static>,
    },
    Notification {
        method: String,
        params: Value<'static>,
    },
    /// An answer to a request of the server's, which it sends none of.
    Response,
}

impl Incoming {
    /// The message `text` holds.
    pub(crate) fn read(text: &str) -> Result<Self, Failure> {
        let value =
            vun_json::parse(text).map_err(|_| Failure::new(Code::ParseError, "the message is not JSON"))?;
        let object =
            value.as_object().ok_or_else(|| Failure::new(Code::InvalidRequest, "a message is an object"))?;
        let params = object.get("params").cloned().unwrap_or_default().into_owned();
        let id = object.get("id").map(|id| Id::read(id).ok_or_else(|| Failure::invalid("id")));
        match (object.get("method").and_then(Value::as_str), id) {
            (Some(method), Some(id)) => Ok(Self::Request { id: id?, method: method.to_owned(), params }),
            (Some(method), None) => Ok(Self::Notification { method: method.to_owned(), params }),
            (None, Some(_)) => Ok(Self::Response),
            (None, None) => {
                Err(Failure::new(Code::InvalidRequest, "a message names a method or answers one"))
            }
        }
    }
}

/// The codes of the errors a response carries: JSON-RPC's, then the protocol's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Code {
    ParseError,
    InvalidRequest,
    MethodNotFound,
    InvalidParams,
    ServerNotInitialized,
    InvalidAfterShutdown,
}

impl Code {
    /// The number the protocol gives it.
    const fn number(self) -> i64 {
        match self {
            Self::ParseError => -32700,
            Self::InvalidRequest | Self::InvalidAfterShutdown => -32600,
            Self::MethodNotFound => -32601,
            Self::InvalidParams => -32602,
            Self::ServerNotInitialized => -32002,
        }
    }
}

/// Why a request failed: its code, and what to show of it.
#[derive(Debug)]
pub(crate) struct Failure {
    pub(crate) code: Code,
    message: &'static str,
}

impl Failure {
    pub(crate) const fn new(code: Code, message: &'static str) -> Self {
        Self { code, message }
    }

    /// Parameters lacking `field`, or holding another shape there.
    pub(crate) const fn invalid(field: &'static str) -> Self {
        Self { code: Code::InvalidParams, message: field }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.code {
            Code::InvalidParams => {
                write!(f, "the parameters lack `{}`, or hold another shape there", self.message)
            }
            _ => f.write_str(self.message),
        }
    }
}

impl std::error::Error for Failure {}

/// A message to the client, as the protocol writes it.
pub(crate) enum Outgoing {
    Response { id: Option<Id>, result: Result<Value<'static>, Failure> },
    Notification { method: &'static str, params: Value<'static> },
}

impl Outgoing {
    pub(crate) fn value(self) -> Value<'static> {
        let object = Object::new().with("jsonrpc", VERSION);
        let object = match self {
            Self::Response { id, result } => {
                let object = object.with("id", id.map_or(Value::Null, |id| id.value()));
                match result {
                    Ok(result) => object.with("result", result),
                    Err(failure) => object.with(
                        "error",
                        Object::new()
                            .with("code", failure.code.number())
                            .with("message", failure.to_string()),
                    ),
                }
            }
            Self::Notification { method, params } => object.with("method", method).with("params", params),
        };
        Value::Object(object)
    }
}
