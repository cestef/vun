//! Why a request failed: the protocol's code for it, and a typed reason, shown to the client as
//! the response's message.

use std::fmt;

use vun_data::Mismatch;
use vun_lsp_protocol::rpc::{ErrorCode, ResponseError};
use vun_uri::NotFile;

use crate::transport::Unframed;

#[derive(Debug)]
pub(crate) struct Failure {
    pub(crate) code: ErrorCode,
    pub(crate) reason: Reason,
}

#[derive(Debug)]
pub(crate) enum Reason {
    /// The message could not be read off the stream.
    Unframed(Unframed),
    /// The message is not JSON.
    Json(vun_data::json::Error),
    /// The message is JSON, but not a message the protocol sends.
    Message(Mismatch),
    /// A message that names no method and answers no request.
    Nothing,
    /// The parameters do not fit the method's.
    Params(Mismatch),
    /// The server has no method of the name.
    Method,
    /// A method other than `initialize` before it.
    Uninitialized,
    /// A method other than `exit` after `shutdown`.
    ShutDown,
    /// The document named is not open.
    Closed,
    /// The document named is not a local file.
    NotLocal(NotFile),
}

impl Failure {
    pub(crate) const fn new(code: ErrorCode, reason: Reason) -> Self {
        Self { code, reason }
    }

    pub(crate) const fn params(mismatch: Mismatch) -> Self {
        Self::new(ErrorCode::InvalidParams, Reason::Params(mismatch))
    }

    /// The error a response carries.
    pub(crate) fn response(&self) -> ResponseError {
        ResponseError { code: self.code, message: self.to_string() }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.reason {
            Reason::Unframed(unframed) => write!(f, "{unframed}"),
            Reason::Json(_) => f.write_str("the message is not JSON"),
            Reason::Message(mismatch) => write!(f, "not a message: {mismatch}"),
            Reason::Nothing => f.write_str("a message names a method or answers a request"),
            Reason::Params(mismatch) => write!(f, "the parameters do not fit: {mismatch}"),
            Reason::Method => f.write_str("the server has no such method"),
            Reason::Uninitialized => f.write_str("the server is not initialized"),
            Reason::ShutDown => f.write_str("the server is shut down"),
            Reason::Closed => f.write_str("the document is not open"),
            Reason::NotLocal(not) => write!(f, "the document is not a local file: {not}"),
        }
    }
}

impl std::error::Error for Failure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.reason {
            Reason::Unframed(e) => Some(e),
            Reason::Json(e) => Some(e),
            Reason::Message(e) | Reason::Params(e) => Some(e),
            Reason::NotLocal(e) => Some(e),
            Reason::Nothing | Reason::Method | Reason::Uninitialized | Reason::ShutDown | Reason::Closed => {
                None
            }
        }
    }
}
