//! What the host and a scheme component say to each other.

use vun_codec::{Decode, Encode, Malformed};
use vun_component::Cause;
use vun_uri::Uri;

/// What the host asks of a scheme: each method of `Scheme`.
#[derive(Encode, Decode)]
pub enum Call {
    Name,
    Read(Uri),
}

#[derive(Encode, Decode)]
pub enum Reply {
    Name(&'static str),
    Body(Vec<u8>),
    /// Why a read failed, as its messages.
    Failed(Cause),
}

impl Reply {
    /// The scheme's name, if this is the reply to `Name`.
    ///
    /// # Errors
    /// Returns [`Malformed::Invariant`] for the reply to another call.
    pub fn name(self) -> Result<&'static str, Malformed> {
        match self {
            Self::Name(name) => Ok(name),
            Self::Body(_) | Self::Failed(_) => Err(Malformed::Invariant),
        }
    }

    /// The bytes read, or why they were not, if this is the reply to `Read`.
    ///
    /// # Errors
    /// Returns [`Malformed::Invariant`] for the reply to another call.
    pub fn read(self) -> Result<Result<Vec<u8>, Cause>, Malformed> {
        match self {
            Self::Body(bytes) => Ok(Ok(bytes)),
            Self::Failed(cause) => Ok(Err(cause)),
            Self::Name(_) => Err(Malformed::Invariant),
        }
    }
}
