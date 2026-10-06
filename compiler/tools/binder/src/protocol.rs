//! What the host and a binder component say to each other.

use vun_codec::{Decode, Encode, Malformed};

use crate::{Bindings, Failure, Request};

/// What the host asks of a binder: each method of `Binder`.
#[derive(Encode, Decode)]
pub enum Call {
    Name,
    Bind(Request),
}

#[derive(Encode, Decode)]
pub enum Reply {
    Name(&'static str),
    Bound(Result<Bindings, Failure>),
}

impl Reply {
    /// The binder's name, if this is the reply to `Name`.
    ///
    /// # Errors
    /// Returns [`Malformed::Invariant`] for the reply to another call.
    pub fn name(self) -> Result<&'static str, Malformed> {
        match self {
            Self::Name(name) => Ok(name),
            Self::Bound(_) => Err(Malformed::Invariant),
        }
    }

    /// The bindings, or why there are none, if this is the reply to `Bind`.
    ///
    /// # Errors
    /// Returns [`Malformed::Invariant`] for the reply to another call.
    pub fn bound(self) -> Result<Result<Bindings, Failure>, Malformed> {
        match self {
            Self::Bound(bound) => Ok(bound),
            Self::Name(_) => Err(Malformed::Invariant),
        }
    }
}
