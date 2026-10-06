//! `cargo xtask size`: the distributed `vun` stays within its budget (decision 131). The budget
//! is 20 MiB (decision 252), raised from 12 MiB (decision 193) once the https client of decision
//! 248 took the build past it; raising it takes a decision, as a new external crate does.

use std::path::{Path, PathBuf};
use std::{fmt, io};

use crate::cargo::{self, CargoFailed};

/// The most bytes the stripped `dist` build of `vun` may take on the host.
pub(crate) const BUDGET: u64 = 26 * 1024 * 1024;

/// The profile what is distributed is built with (`[profile.dist]`).
const PROFILE: &str = "dist";

/// Why the binary could not be measured.
#[derive(Debug)]
pub(crate) enum Unmeasured {
    Cargo(CargoFailed),
    Unreadable(io::Error),
}

impl fmt::Display for Unmeasured {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cargo(_) => write!(f, "cargo cannot build `vun-cli`"),
            Self::Unreadable(_) => f.write_str("the executable cannot be read"),
        }
    }
}

impl std::error::Error for Unmeasured {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Cargo(e) => Some(e),
            Self::Unreadable(e) => Some(e),
        }
    }
}

/// The measured binary.
pub(crate) struct Measured {
    pub(crate) path: PathBuf,
    pub(crate) bytes: u64,
}

impl Measured {
    /// Builds `vun` as it is distributed from the workspace at `root` and measures it.
    ///
    /// # Errors
    /// Returns why cargo cannot run or fails, or names no executable.
    pub(crate) fn build(root: &Path) -> Result<Self, Unmeasured> {
        let path = cargo::vun(root, PROFILE).map_err(Unmeasured::Cargo)?;
        let bytes = std::fs::metadata(&path).map_err(Unmeasured::Unreadable)?.len();
        Ok(Self { path, bytes })
    }
}
