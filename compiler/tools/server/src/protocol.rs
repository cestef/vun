//! What the host and a server component say to each other.

use vun_codec::{Decode, Encode, Malformed};
use vun_command::Outcome;
use vun_component::Cause;
use vun_diag::DiagCtxt;

/// What the host asks of a server: each method of `Server`.
#[derive(Encode, Decode)]
pub enum Call {
    Name,
    Serve,
}

#[derive(Encode, Decode)]
pub enum Reply {
    Name(&'static str),
    /// How the client's session ended, with what the server reported.
    Served {
        outcome: Outcome,
        diag: DiagCtxt,
    },
    /// Why talking to the client failed.
    Failed(Cause),
}

impl Reply {
    /// The server's name, if this is the reply to `Name`.
    ///
    /// # Errors
    /// Returns [`Malformed::Invariant`] for the reply to another call.
    pub fn name(self) -> Result<&'static str, Malformed> {
        match self {
            Self::Name(name) => Ok(name),
            Self::Served { .. } | Self::Failed(_) => Err(Malformed::Invariant),
        }
    }

    /// How the session ended, or why it could not, if this is the reply to `Serve`.
    ///
    /// # Errors
    /// Returns [`Malformed::Invariant`] for the reply to another call.
    pub fn served(self) -> Result<Result<(Outcome, DiagCtxt), Cause>, Malformed> {
        match self {
            Self::Served { outcome, diag } => Ok(Ok((outcome, diag))),
            Self::Failed(cause) => Ok(Err(cause)),
            Self::Name(_) => Err(Malformed::Invariant),
        }
    }
}
