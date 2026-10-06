//! What the host and a backend component say to each other.

use std::borrow::Cow;

use vun_backend::{BackendInfo, Emitted, Lines};
use vun_codec::{Decode, Encode, Malformed};
use vun_diag::DiagCtxt;
use vun_intern::Symbol;
use vun_lir::FuncId;
use vun_lir::unit::Unit;
use vun_lir_passes::legalize::Division;
use vun_origin::OriginId;
use vun_target::{Profile, TargetSpec};

/// What the host asks of a backend: each method of `Backend`.
#[derive(Encode, Decode)]
pub enum Call<'a> {
    Info,
    Emit(Box<Emit<'a>>),
    Division(Cow<'a, TargetSpec>),
    FrameSizes { unit: Cow<'a, Unit>, report: Cow<'a, [u8]> },
}

/// What a backend is asked to generate code for: a [`vun_backend::Request`] without its lines,
/// which the backend asks for as it needs them.
#[derive(Encode, Decode)]
pub struct Emit<'a> {
    pub unit: Cow<'a, Unit>,
    pub target: Cow<'a, TargetSpec>,
    pub profile: Profile,
    pub listing: bool,
}

#[derive(Encode, Decode)]
pub enum Reply {
    Info(BackendInfo),
    /// What `emit` produced, its diagnostics, and whether it returned an error.
    Emitted {
        emitted: Emitted,
        diag: DiagCtxt,
        failed: bool,
    },
    Division(Division),
    FrameSizes(Vec<(FuncId, u64)>),
}

/// What a backend asks of the host while it emits: each method of `Lines`.
#[derive(Debug, Encode, Decode)]
pub enum Ask {
    Line(OriginId),
    Declaration(OriginId),
    Inlined(OriginId),
}

#[derive(Debug, Encode, Decode)]
pub enum Answer {
    Line(Option<vun_backend::Line>),
    Declaration(Vec<Symbol>),
    Inlined(Option<(OriginId, OriginId)>),
}

impl Ask {
    /// The answer `lines` gives.
    #[must_use]
    pub fn answered_by(&self, lines: &dyn Lines) -> Answer {
        match *self {
            Self::Line(origin) => Answer::Line(lines.line(origin)),
            Self::Declaration(origin) => Answer::Declaration(lines.declaration(origin)),
            Self::Inlined(origin) => Answer::Inlined(lines.inlined(origin)),
        }
    }
}

impl Reply {
    /// The facts of a backend, if this is the reply to `Info`.
    ///
    /// # Errors
    /// Returns [`Malformed::Invariant`] for the reply to another call.
    pub fn info(self) -> Result<BackendInfo, Malformed> {
        match self {
            Self::Info(info) => Ok(info),
            _ => Err(Malformed::Invariant),
        }
    }

    /// What was emitted, the diagnostics and whether it failed, if this answers `Emit`.
    ///
    /// # Errors
    /// As [`Reply::info`].
    pub fn emitted(self) -> Result<(Emitted, DiagCtxt, bool), Malformed> {
        match self {
            Self::Emitted { emitted, diag, failed } => Ok((emitted, diag, failed)),
            _ => Err(Malformed::Invariant),
        }
    }

    /// How integers divide, if this answers `Division`.
    ///
    /// # Errors
    /// As [`Reply::info`].
    pub fn division(self) -> Result<Division, Malformed> {
        match self {
            Self::Division(division) => Ok(division),
            _ => Err(Malformed::Invariant),
        }
    }

    /// The frame sizes, if this answers `FrameSizes`.
    ///
    /// # Errors
    /// As [`Reply::info`].
    pub fn frame_sizes(self) -> Result<Vec<(FuncId, u64)>, Malformed> {
        match self {
            Self::FrameSizes(sizes) => Ok(sizes),
            _ => Err(Malformed::Invariant),
        }
    }
}

impl Answer {
    /// The line, if this answers `Line`.
    ///
    /// # Errors
    /// Returns [`Malformed::Invariant`] for the answer to another question.
    pub fn line(self) -> Result<Option<vun_backend::Line>, Malformed> {
        match self {
            Self::Line(line) => Ok(line),
            _ => Err(Malformed::Invariant),
        }
    }

    /// The declaration's path, if this answers `Declaration`.
    ///
    /// # Errors
    /// As [`Answer::line`].
    pub fn declaration(self) -> Result<Vec<Symbol>, Malformed> {
        match self {
            Self::Declaration(path) => Ok(path),
            _ => Err(Malformed::Invariant),
        }
    }

    /// The inlined copy, if this answers `Inlined`.
    ///
    /// # Errors
    /// As [`Answer::line`].
    pub fn inlined(self) -> Result<Option<(OriginId, OriginId)>, Malformed> {
        match self {
            Self::Inlined(copy) => Ok(copy),
            _ => Err(Malformed::Invariant),
        }
    }
}
