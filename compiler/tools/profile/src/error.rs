//! Why a measurement was not taken.

use std::error::Error;
use std::fmt;
use std::io;
use std::process::{ExitStatus, Output};
use std::time::Duration;

use vun_data::{Mismatch, json};

use crate::callgrind::ParseError;

#[derive(Debug)]
pub enum MeasureError {
    /// A program cannot be started: the compiler, or `valgrind`.
    Spawn(&'static str, io::Error),
    Io(io::Error),
    /// What was run ended without success.
    Failed(&'static str, ExitStatus, Written),
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
            Self::Failed(what, status, _) => write!(f, "{what} ended with {status}"),
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
            Self::Failed(.., written) => Some(written),
            Self::TimedOut(..) | Self::Unreported => None,
        }
    }
}

/// The last lines a failed run wrote to its error stream.
#[derive(Debug)]
pub struct Written(String);

impl Written {
    const LINES: usize = 20;

    #[must_use]
    pub fn of(output: &Output) -> Self {
        let text = String::from_utf8_lossy(&output.stderr);
        let lines: Vec<&str> = text.lines().collect();
        Self(lines[lines.len().saturating_sub(Self::LINES)..].join("\n"))
    }
}

impl fmt::Display for Written {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for Written {}

impl From<io::Error> for MeasureError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
