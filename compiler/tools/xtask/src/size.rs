//! `cargo xtask size`: the distributed `vun` stays within its budget (decision 131). The budget
//! is 20 MiB (decision 252), raised from 12 MiB (decision 193) once the https client of decision
//! 248 took the build past it; raising it takes a decision, as a new external crate does.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::{fmt, io};

/// The most bytes the stripped `dist` build of `vun` may take on the host.
pub(crate) const BUDGET: u64 = 20 * 1024 * 1024;

/// The package whose binary is measured.
const PACKAGE: &str = "vun-cli";
/// The profile what is distributed is built with (`[profile.dist]`).
const PROFILE: &str = "dist";

/// Why the binary could not be measured.
#[derive(Debug)]
pub(crate) enum Unmeasured {
    Cargo(io::Error),
    Failed(ExitStatus),
    NoExecutable,
    Unreadable(io::Error),
}

impl fmt::Display for Unmeasured {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cargo(_) => f.write_str("cargo cannot run"),
            Self::Failed(status) => write!(f, "`cargo build --profile {PROFILE}` failed: {status}"),
            Self::NoExecutable => write!(f, "cargo built no executable of `{PACKAGE}`"),
            Self::Unreadable(_) => f.write_str("the executable cannot be read"),
        }
    }
}

impl std::error::Error for Unmeasured {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Cargo(e) | Self::Unreadable(e) => Some(e),
            Self::Failed(_) | Self::NoExecutable => None,
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
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let output = Command::new(cargo)
            .current_dir(root)
            .args(["build", "--quiet", "--message-format=json", "--profile", PROFILE, "--package", PACKAGE])
            .stderr(Stdio::inherit())
            .output()
            .map_err(Unmeasured::Cargo)?;
        if !output.status.success() {
            return Err(Unmeasured::Failed(output.status));
        }
        let messages = String::from_utf8_lossy(&output.stdout);
        let path = Self::executable(&messages).ok_or(Unmeasured::NoExecutable)?;
        let bytes = std::fs::metadata(&path).map_err(Unmeasured::Unreadable)?.len();
        Ok(Self { path, bytes })
    }

    /// The executable the last artifact message of cargo's JSON lines names, if any.
    pub(crate) fn executable(messages: &str) -> Option<PathBuf> {
        messages
            .lines()
            .filter_map(|line| vun_json::parse(line).ok())
            .filter(|m| m["reason"].as_str() == Some("compiler-artifact"))
            .filter_map(|m| m["executable"].as_str().map(PathBuf::from))
            .next_back()
    }
}
