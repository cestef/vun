//! The messages a client sends, told apart by the members they hold.

use vun_data::Value;
use vun_lsp_protocol::rpc::{ErrorCode, Id, Message};

use crate::failure::{Failure, Reason};

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
    /// An answer to a request of the server's.
    Response,
}

impl Incoming {
    /// The message `text` holds.
    pub(crate) fn read(text: &str) -> Result<Self, Failure> {
        let value =
            vun_data::json::parse(text).map_err(|e| Failure::new(ErrorCode::ParseError, Reason::Json(e)))?;
        let message: Message =
            value.read().map_err(|m| Failure::new(ErrorCode::InvalidRequest, Reason::Message(m)))?;
        let params = message.params.unwrap_or_default();
        match (message.method, message.id) {
            (Some(method), Some(id)) => Ok(Self::Request { id, method, params }),
            (Some(method), None) => Ok(Self::Notification { method, params }),
            (None, Some(_)) => Ok(Self::Response),
            (None, None) => Err(Failure::new(ErrorCode::InvalidRequest, Reason::Nothing)),
        }
    }
}
