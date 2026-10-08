//! What a trial of a program found wrong, as typed values: a program made to be accepted,
//! lowered and run alike by every backend that is not.

use std::error::Error;
use std::fmt;

use vun_diag::DiagId;
use vun_mir::Check;

use crate::compare::Difference;
use crate::crash::Site;

/// Where in the compiler's work a finding was made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Reading, resolving and typing the file.
    Check,
    /// MIR, built and verified.
    Mir,
    /// LIR, lowered and verified.
    Lir,
    /// Code, linking and the run of the program, through a backend.
    Backend(&'static str),
}

/// An error a program was refused with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub id: DiagId,
    /// What it says, for people.
    pub message: String,
}

/// What a stage does not handle yet, as the stage names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lacking {
    /// A form MIR does not build, and the check of the builder that stopped.
    Mir(Check, vun_mir::Why),
    /// A form LIR does not lower.
    Lir(vun_lower::Why),
}

/// How a program that ran did not end well.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Death {
    /// With a status other than success.
    Exit(i32),
    /// By a trap, whichever way its backend traps.
    Trapped,
    /// By a signal that is no trap: a fault of the memory, a kill.
    Crashed(i32),
    /// It was still running at the limit.
    Hung,
}

/// A backend's program parting from the reference, the run most of the backends agree with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parting {
    pub backend: &'static str,
    pub difference: Difference,
}

/// What was wrong. Each is a bug of the compiler, or of the program made, which the report of
/// the finding says apart by what the model knows of the program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Finding {
    /// The compiler panicked.
    Crashed { stage: Stage, site: Site },
    /// The checks refused the program.
    Rejected(Vec<Report>),
    /// An IR the compiler made does not verify.
    Malformed { stage: Stage, reason: String },
    /// The program does not lower, in MIR or in LIR.
    Unlowered { stage: Stage, lacking: Lacking },
    /// A backend or the linker refused to build it.
    Unbuilt { backend: &'static str, reports: Vec<Report> },
    /// A program ended other than by success.
    Died { backend: &'static str, death: Death },
    /// Backends that built and ran it did not agree; the reference is the run most agree with.
    Parted { reference: &'static str, partings: Vec<Parting> },
}

impl Finding {
    /// Whether a backend said it cannot express the program (`backend.lacks`) or has no tool to
    /// build it with (`backend.tool`): a limit it declares, not a fault.
    #[must_use]
    pub fn is_lack(&self) -> bool {
        matches!(self, Self::Unbuilt { reports, .. }
            if !reports.is_empty()
                && reports.iter().all(|r| [vun_diag::errors::BACKEND_LACKS, vun_diag::errors::BACKEND_TOOL].contains(&r.id)))
    }

    /// The stage the finding was made in.
    #[must_use]
    pub fn stage(&self) -> Stage {
        match self {
            Self::Crashed { stage, .. } | Self::Malformed { stage, .. } | Self::Unlowered { stage, .. } => {
                stage.clone()
            }
            Self::Rejected(_) => Stage::Check,
            Self::Unbuilt { backend, .. } | Self::Died { backend, .. } => Stage::Backend(backend),
            Self::Parted { reference, .. } => Stage::Backend(reference),
        }
    }
}

/// What a finding is about, apart from where in the program it was made: the findings of two
/// programs with one cause are alike, which is what reducing a program keeps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cause {
    Crashed(Stage, Site),
    Rejected(Vec<DiagId>),
    Malformed(Stage),
    Unlowered(Stage, Lacking),
    Unbuilt(&'static str, Vec<DiagId>),
    Died(&'static str, Death),
    Parted(&'static str, Vec<&'static str>),
}

impl Finding {
    /// What the finding is about.
    #[must_use]
    pub fn cause(&self) -> Cause {
        let ids = |reports: &[Report]| reports.iter().map(|r| r.id).collect();
        match self {
            Self::Crashed { stage, site } => Cause::Crashed(stage.clone(), site.clone()),
            Self::Rejected(reports) => Cause::Rejected(ids(reports)),
            Self::Malformed { stage, .. } => Cause::Malformed(stage.clone()),
            Self::Unlowered { stage, lacking } => Cause::Unlowered(stage.clone(), lacking.clone()),
            Self::Unbuilt { backend, reports } => Cause::Unbuilt(backend, ids(reports)),
            Self::Died { backend, death } => Cause::Died(backend, *death),
            Self::Parted { reference, partings } => {
                Cause::Parted(reference, partings.iter().map(|p| p.backend).collect())
            }
        }
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Check => f.write_str("check"),
            Self::Mir => f.write_str("mir"),
            Self::Lir => f.write_str("lir"),
            Self::Backend(id) => write!(f, "backend {id}"),
        }
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Crashed { stage, site } => write!(f, "the compiler crashed in {stage} at {site}"),
            Self::Rejected(reports) => {
                write!(f, "refused with")?;
                reports.iter().try_for_each(|r| write!(f, " {}", r.id))
            }
            Self::Malformed { stage, reason } => write!(f, "{stage} does not verify: {reason}"),
            Self::Unlowered { stage, lacking } => write!(f, "{stage} does not lower it: {lacking:?}"),
            Self::Unbuilt { backend, reports } => {
                write!(f, "{backend} cannot build it:")?;
                reports.iter().try_for_each(|r| write!(f, " {}", r.id))
            }
            Self::Died { backend, death } => write!(f, "the program of {backend} died: {death:?}"),
            Self::Parted { reference, partings } => {
                write!(f, "backends part from {reference}:")?;
                partings.iter().try_for_each(|p| write!(f, " {}", p.backend))
            }
        }
    }
}

impl Error for Finding {}
