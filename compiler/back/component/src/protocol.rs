//! What the host and a backend component say to each other.

use std::borrow::Cow;

use vun_backend::{Assist, Assisted, BackendInfo, Emitted, Lines, Request};
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
    FrameSizes {
        unit: Cow<'a, Unit>,
        report: Cow<'a, [u8]>,
    },
    /// Whether the backend compiles for another's functions, for a target.
    Assists(Cow<'a, TargetSpec>),
    /// The code of functions another backend cannot express, of a unit to emit.
    Assist {
        emit: Box<Emit<'a>>,
        ids: Vec<FuncId>,
    },
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

impl<'a> Emit<'a> {
    /// The request this asks, over the host's `asking` for lines and assistance.
    #[must_use]
    pub fn request<'r>(&'r self, asking: &'r (impl Lines + Assist)) -> Request<'r> {
        Request {
            unit: &self.unit,
            target: &self.target,
            lines: asking,
            assist: asking,
            profile: self.profile,
            listing: self.listing,
        }
    }

    /// What `request` asks, to send.
    #[must_use]
    pub fn of(request: &Request<'a>) -> Self {
        Self {
            unit: Cow::Borrowed(request.unit),
            target: Cow::Borrowed(request.target),
            profile: request.profile,
            listing: request.listing,
        }
    }
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
    Assists(bool),
    /// The code, or none if the backend does not assist.
    Assisted(Option<Assisted>),
}

/// What a backend asks of the host while it emits: each method of `Lines` and of the request's
/// `Assist`.
#[derive(Debug, Encode, Decode)]
pub enum Ask {
    Line(OriginId),
    Declaration(OriginId),
    Inlined(OriginId),
    Assists(Box<TargetSpec>),
    Assist(Vec<FuncId>),
}

#[derive(Encode, Decode)]
pub enum Answer {
    Line(Option<vun_backend::Line>),
    Declaration(Vec<Symbol>),
    Inlined(Option<(OriginId, OriginId)>),
    Assists(bool),
    Assisted(Assisted),
}

impl Ask {
    /// The answer the request's lines and assist give.
    #[must_use]
    pub fn answered_by(&self, request: &Request<'_>) -> Answer {
        match self {
            Self::Line(origin) => Answer::Line(request.lines.line(*origin)),
            Self::Declaration(origin) => Answer::Declaration(request.lines.declaration(*origin)),
            Self::Inlined(origin) => Answer::Inlined(request.lines.inlined(*origin)),
            Self::Assists(target) => Answer::Assists(request.assist.assists(target)),
            Self::Assist(ids) => Answer::Assisted(request.assist.compile(request, ids)),
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

impl Reply {
    /// Whether the backend assists, if this answers `Assists`.
    ///
    /// # Errors
    /// As [`Reply::info`].
    pub fn assists(self) -> Result<bool, Malformed> {
        match self {
            Self::Assists(yes) => Ok(yes),
            _ => Err(Malformed::Invariant),
        }
    }

    /// The code it made, if this answers `Assist` and the backend assists.
    ///
    /// # Errors
    /// As [`Reply::info`]; also if the backend does not assist.
    pub fn assisted(self) -> Result<Assisted, Malformed> {
        match self {
            Self::Assisted(Some(code)) => Ok(code),
            _ => Err(Malformed::Invariant),
        }
    }
}

impl Answer {
    /// Whether the host's assist compiles for the target, if this answers `Assists`.
    ///
    /// # Errors
    /// Returns [`Malformed::Invariant`] for the answer to another question.
    pub fn assists(self) -> Result<bool, Malformed> {
        match self {
            Self::Assists(yes) => Ok(yes),
            _ => Err(Malformed::Invariant),
        }
    }

    /// The code the host's assist made, if this answers `Assist`.
    ///
    /// # Errors
    /// As [`Answer::assists`].
    pub fn assisted(self) -> Result<Assisted, Malformed> {
        match self {
            Self::Assisted(code) => Ok(code),
            _ => Err(Malformed::Invariant),
        }
    }

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
