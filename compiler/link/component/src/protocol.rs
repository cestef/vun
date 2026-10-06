//! What the host asks of a linker component: each method of `Linker`. A linker asks nothing back.

use std::borrow::Cow;

use vun_codec::{Decode, Encode, Malformed};
use vun_diag::DiagCtxt;
use vun_link::{LinkPlan, LinkerInfo};
use vun_target::{Run, TargetSpec};

#[derive(Encode, Decode)]
pub enum Call<'a> {
    Info,
    Runs(Cow<'a, TargetSpec>),
    Link { plan: Cow<'a, LinkPlan>, target: Cow<'a, TargetSpec> },
}

#[derive(Encode, Decode)]
pub enum Reply {
    Info(LinkerInfo),
    Runs(Vec<Run>),
    /// The diagnostics of a link, and whether it returned an error.
    Linked {
        diag: DiagCtxt,
        failed: bool,
    },
}

impl Reply {
    /// The facts of a linker, if this is the reply to `Info`.
    ///
    /// # Errors
    /// Returns [`Malformed::Invariant`] for the reply to another call.
    pub fn info(self) -> Result<LinkerInfo, Malformed> {
        match self {
            Self::Info(info) => Ok(info),
            _ => Err(Malformed::Invariant),
        }
    }

    /// The programs it runs, if this answers `Runs`.
    ///
    /// # Errors
    /// As [`Reply::info`].
    pub fn runs(self) -> Result<Vec<Run>, Malformed> {
        match self {
            Self::Runs(runs) => Ok(runs),
            _ => Err(Malformed::Invariant),
        }
    }

    /// The diagnostics and whether it failed, if this answers `Link`.
    ///
    /// # Errors
    /// As [`Reply::info`].
    pub fn linked(self) -> Result<(DiagCtxt, bool), Malformed> {
        match self {
            Self::Linked { diag, failed } => Ok((diag, failed)),
            _ => Err(Malformed::Invariant),
        }
    }
}
