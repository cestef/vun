//! Why a measurement was not taken.

use std::error::Error;
use std::fmt;
use std::io;
use std::process::ExitStatus;
use std::time::Duration;

use vun_data::{Mismatch, json};

use crate::callgrind::ParseError;

#[derive(Debug)]
pub enum MeasureError {
    /// A program cannot be started: the compiler, or `valgrind`.
    Spawn(&'static str, io::Error),
    Io(io::Error),
    /// What was run ended without success.
    Failed(&'static str, ExitStatus),
    /// What was run was still running at the limit.
    TimedOut(&'static str, Duration),
    /// The compiler reported no timings.
    Unreported,
    Json(json::Error),
    Shape(Mismatch),
    Callgrind(ParseError),
}

impl fmt::Display for MeasureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(program, _) => write!(f, "`{program}` does not start"),
            Self::Io(_) => write!(f, "a file of the measurement is not read or written"),
            Self::Failed(what, status) => write!(f, "{what} ended with {status}"),
            Self::TimedOut(what, limit) => write!(f, "{what} was still running after {limit:?}"),
            Self::Unreported => write!(f, "the compiler wrote no timings"),
            Self::Json(_) | Self::Shape(_) => write!(f, "the compiler's timings are no timings"),
            Self::Callgrind(_) => write!(f, "callgrind's profile is not read"),
        }
    }
}

impl Error for MeasureError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Spawn(_, source) | Self::Io(source) => Some(source),
            Self::Json(source) => Some(source),
            Self::Shape(source) => Some(source),
            Self::Callgrind(source) => Some(source),
            Self::Failed(..) | Self::TimedOut(..) | Self::Unreported => None,
        }
    }
}

impl From<io::Error> for MeasureError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
