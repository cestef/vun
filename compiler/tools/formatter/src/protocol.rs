//! What the host and a formatter component say to each other.

use std::borrow::Cow;

use vun_codec::{Decode, Encode, Malformed};

use crate::Unlaid;

/// What the host asks of a formatter: each method of `Formatter`.
#[derive(Encode, Decode)]
pub enum Call<'a> {
    Name,
    Layout { text: Cow<'a, str>, width: u64 },
}

#[derive(Encode, Decode)]
pub enum Reply {
    Name(&'static str),
    Laid(Result<String, Unlaid>),
}

impl Reply {
    /// The formatter's name, if this is the reply to `Name`.
    ///
    /// # Errors
    /// Returns [`Malformed::Invariant`] for the reply to another call.
    pub fn name(self) -> Result<&'static str, Malformed> {
        match self {
            Self::Name(name) => Ok(name),
            Self::Laid(_) => Err(Malformed::Invariant),
        }
    }

    /// The text laid out, or why it is not, if this is the reply to `Layout`.
    ///
    /// # Errors
    /// Returns [`Malformed::Invariant`] for the reply to another call.
    pub fn laid(self) -> Result<Result<String, Unlaid>, Malformed> {
        match self {
            Self::Laid(laid) => Ok(laid),
            Self::Name(_) => Err(Malformed::Invariant),
        }
    }
}
