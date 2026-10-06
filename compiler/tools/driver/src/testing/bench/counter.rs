//! Counting the instructions a process retires, through `perf stat` (decision 601): the program
//! runs under it, which writes the count to a file.

use std::error::Error;
use std::fmt;
use std::io;
use std::path::Path;
use std::process::Command;

use vun_data::ToData;

use crate::testing::{Collected, Run};

/// The program counting.
const PERF: &str = "perf";
/// The event it counts, in user mode.
const EVENT: &str = "instructions:u";
/// What separates the fields of a line `perf stat` writes with `-x`.
const SEPARATOR: &str = ",";

/// Why instructions are not counted here.
#[derive(Debug)]
pub(super) enum Unavailable {
    /// Only Linux has `perf`.
    Platform,
    /// The program runs through an emulator, which would be counted instead.
    Runner,
    /// `perf` does not start.
    Missing(io::Error),
    /// `perf` ran and counted nothing: the machine has no counter for the event, or the system's
    /// `perf_event_paranoid` does not let this user read it.
    NoCounter,
}

/// What `--measure instructions` fell back from, as a record of the run says it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ToData)]
pub(super) enum Reason {
    Platform,
    Runner,
    Missing,
    NoCounter,
}

impl Unavailable {
    #[must_use]
    pub(super) const fn reason(&self) -> Reason {
        match self {
            Self::Platform => Reason::Platform,
            Self::Runner => Reason::Runner,
            Self::Missing(_) => Reason::Missing,
            Self::NoCounter => Reason::NoCounter,
        }
    }
}

impl fmt::Display for Unavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Platform => write!(f, "instructions are counted on Linux only"),
            Self::Runner => {
                write!(f, "the target's programs run through an emulator, which would be counted")
            }
            Self::Missing(_) => write!(f, "`{PERF}` does not start"),
            Self::NoCounter => write!(f, "`{PERF}` has no counter for {EVENT} here, or may not read it"),
        }
    }
}

impl Error for Unavailable {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Missing(source) => Some(source),
            Self::Platform | Self::Runner | Self::NoCounter => None,
        }
    }
}

/// What a process wrote, and the instructions it retired.
pub(super) struct Counted {
    pub collected: Collected,
    pub instructions: u64,
}

/// Counting through `perf`, into a file.
pub(super) struct Counter<'a> {
    pub into: &'a Path,
}

impl Counter<'_> {
    /// Whether this machine and target can be counted at all, before a process starts.
    ///
    /// # Errors
    /// Why not.
    pub(super) fn allowed(emulated: bool) -> Result<(), Unavailable> {
        if !cfg!(target_os = "linux") {
            return Err(Unavailable::Platform);
        }
        if emulated {
            return Err(Unavailable::Runner);
        }
        Ok(())
    }

    /// Runs `command`, which runs `program`, under `perf stat` and collects what it writes, with
    /// the count of its instructions.
    ///
    /// # Errors
    /// Fails when the output cannot be read; `Err` inside says why nothing is counted.
    pub(super) fn run(&self, program: &Path, command: &Command) -> io::Result<Result<Counted, Unavailable>> {
        let mut counted = Command::new(PERF);
        counted.args(["stat", "-x", SEPARATOR, "-e", EVENT, "-o"]).arg(self.into).arg("--");
        counted.arg(command.get_program()).args(command.get_args());
        let run = match Run::spawn(program, counted) {
            Ok(run) => run,
            Err(e) => return Ok(Err(Unavailable::Missing(e))),
        };
        let collected = run.collect(&mut |_| {})?;
        let count = std::fs::read_to_string(self.into).ok().and_then(|text| Self::parse(&text));
        Ok(count.map(|instructions| Counted { collected, instructions }).ok_or(Unavailable::NoCounter))
    }

    /// The count in the lines `perf stat -x` writes: the first field of the line of the event,
    /// which says `<not supported>` or `<not counted>` where the machine has no counter.
    pub(super) fn parse(text: &str) -> Option<u64> {
        let line = text.lines().find(|line| line.contains(EVENT))?;
        line.split(SEPARATOR).next()?.trim().parse().ok()
    }
}
