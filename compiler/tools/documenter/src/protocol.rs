//! What the host and a documenter component say to each other.

use std::borrow::Cow;

use vun_codec::{Decode, Encode, Malformed};
use vun_component::Cause;

use crate::{Site, Source};

/// What the host asks of a documenter: each method of `Documenter`.
#[derive(Encode, Decode)]
pub enum Call<'a> {
    Name,
    Document(Cow<'a, [Source]>),
}

#[derive(Encode, Decode)]
pub enum Reply {
    Name(&'static str),
    Documented(Result<Site, Cause>),
}

/// What a documenter asks of the host while it documents: each method of `Types`.
#[derive(Debug, Encode, Decode)]
pub enum Ask {
    IsType { path: Vec<String>, name: String },
}

#[derive(Encode, Decode)]
pub enum Answer {
    IsType(bool),
}

impl Reply {
    /// The documenter's name, if this is the reply to `Name`.
    ///
    /// # Errors
    /// Returns [`Malformed::Invariant`] for the reply to another call.
    pub fn name(self) -> Result<&'static str, Malformed> {
        match self {
            Self::Name(name) => Ok(name),
            Self::Documented(_) => Err(Malformed::Invariant),
        }
    }

    /// The documentation, or why there is none, if this is the reply to `Document`.
    ///
    /// # Errors
    /// Returns [`Malformed::Invariant`] for the reply to another call.
    pub fn documented(self) -> Result<Result<Site, Cause>, Malformed> {
        match self {
            Self::Documented(site) => Ok(site),
            Self::Name(_) => Err(Malformed::Invariant),
        }
    }
}

impl Answer {
    /// Whether it is a type, if this answers `IsType`.
    #[must_use]
    pub const fn is_type(&self) -> bool {
        let Self::IsType(yes) = self;
        *yes
    }
}
