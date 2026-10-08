//! How a program ended, as the language defines it: it returned a code, it trapped, or it
//! crashed. Which of two backends' signals is a trap is not the program's business: a `ud2`, a
//! `brk` and a call of `abort` are one ending.

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;

/// A signal by which a trapping program ends, whichever way its backend traps (Linux and macOS
/// number these alike).
#[derive(Clone, Copy, Debug, PartialEq, Eq, vun_variants::Variants)]
#[repr(i32)]
pub enum Trap {
    /// An undefined instruction, as `ud2` and `udf` are.
    Illegal = 4,
    /// A breakpoint instruction, as `int3` and `brk` are.
    Breakpoint = 5,
    /// A call of `abort`.
    Abort = 6,
    /// An arithmetic fault the hardware reports.
    Arithmetic = 8,
}

impl Trap {
    /// The trap a signal number is, if it is one.
    #[must_use]
    pub fn of(signal: i32) -> Option<Self> {
        Self::ALL.iter().copied().find(|trap| *trap as i32 == signal)
    }
}

/// How a run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ending {
    /// It returned from `start` with this code.
    Returned(i32),
    /// It stopped itself at a check, by whichever trap its backend uses.
    Trapped,
    /// A signal that no trap is ended it: a fault of the memory, a kill.
    Crashed(i32),
}

impl Ending {
    #[must_use]
    pub fn of(status: &ExitStatus) -> Self {
        match (status.code(), Self::signal(*status)) {
            (Some(code), _) => Self::Returned(code),
            (None, Some(signal)) => Trap::of(signal).map_or(Self::Crashed(signal), |_| Self::Trapped),
            (None, None) => Self::Crashed(0),
        }
    }

    /// The signal that ended a process; a platform without signals has none.
    #[cfg(unix)]
    fn signal(status: ExitStatus) -> Option<i32> {
        status.signal()
    }

    #[cfg(not(unix))]
    fn signal(_: ExitStatus) -> Option<i32> {
        None
    }

    /// Whether it ended on its own terms, so that what it wrote is all of what it would have.
    #[must_use]
    pub const fn returned(self) -> bool {
        matches!(self, Self::Returned(_))
    }
}
