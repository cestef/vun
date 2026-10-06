//! What the host and a renderer component say to each other.

use std::borrow::Cow;

use vun_codec::{Decode, Encode, Malformed};
use vun_diag::Diagnostic;
use vun_origin::Resolve;
use vun_span::FileId;

/// What the host asks of a renderer.
#[derive(Encode, Decode)]
pub enum Call<'a> {
    Info,
    Render(Box<Show<'a>>),
    /// The run is over: what the renderer kept is asked for.
    Finish,
}

/// A diagnostic to show, with colors or without.
#[derive(Encode, Decode)]
pub struct Show<'a> {
    pub diagnostic: Cow<'a, Diagnostic>,
    pub colored: bool,
}

/// What a renderer is called, and what it shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Info {
    pub name: &'static str,
    pub about: &'static str,
}

#[derive(Encode, Decode)]
pub enum Reply {
    Info(Info),
    /// What the sink wrote.
    Rendered(Vec<u8>),
}

/// What a renderer asks of the host while it renders: each method of `Resolve` that depends on
/// the files the host reads now.
#[derive(Debug, Encode, Decode)]
pub enum Ask {
    Current(FileId),
}

#[derive(Debug, Encode, Decode)]
pub enum Answer {
    Current(FileId),
}

impl Ask {
    /// The answer `resolve` gives.
    #[must_use]
    pub fn answered_by(&self, resolve: &dyn Resolve) -> Answer {
        match *self {
            Self::Current(file) => Answer::Current(resolve.current(file)),
        }
    }
}

impl Answer {
    /// The file the host reads under the key asked about, if this answers `Current`.
    ///
    /// # Errors
    /// Returns [`Malformed::Invariant`] for the answer to another question.
    pub fn current(self) -> Result<FileId, Malformed> {
        match self {
            Self::Current(file) => Ok(file),
        }
    }
}

impl Reply {
    /// The facts of a renderer, if this is the reply to `Info`.
    ///
    /// # Errors
    /// Returns [`Malformed::Invariant`] for the reply to another call.
    pub fn info(self) -> Result<Info, Malformed> {
        match self {
            Self::Info(info) => Ok(info),
            Self::Rendered(_) => Err(Malformed::Invariant),
        }
    }

    /// What was written, if this is the reply to `Render`.
    ///
    /// # Errors
    /// Returns [`Malformed::Invariant`] for the reply to another call.
    pub fn rendered(self) -> Result<Vec<u8>, Malformed> {
        match self {
            Self::Rendered(bytes) => Ok(bytes),
            Self::Info(_) => Err(Malformed::Invariant),
        }
    }
}
