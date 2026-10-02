//! Running cargo on the workspace, for the checks that read what it reports.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::{fmt, io};

use crate::task::Program;

/// The package whose `vun` executable a build makes.
const PACKAGE: &str = "vun-cli";

/// Why cargo reported nothing.
#[derive(Debug)]
pub(crate) enum CargoFailed {
    Unstarted(io::Error),
    Failed {
        args: Vec<&'static str>,
        status: ExitStatus,
    },
    /// It built, but named no executable of the package.
    NoExecutable,
}

impl fmt::Display for CargoFailed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unstarted(_) => f.write_str("cargo cannot run"),
            Self::Failed { args, status } => write!(f, "`cargo {}` failed: {status}", args.join(" ")),
            Self::NoExecutable => write!(f, "cargo built no executable of `{PACKAGE}`"),
        }
    }
}

impl std::error::Error for CargoFailed {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unstarted(e) => Some(e),
            Self::Failed { .. } | Self::NoExecutable => None,
        }
    }
}

/// Builds the workspace's `vun` in `profile` at `root` and returns the executable cargo reports,
/// the last compiler-artifact message holding one.
///
/// # Errors
/// Returns why cargo cannot run or fails, or built no executable.
pub(crate) fn vun(root: &Path, profile: &'static str) -> Result<PathBuf, CargoFailed> {
    let args = ["build", "--message-format=json", "--profile", profile, "--package", PACKAGE];
    let messages = output(root, &args)?;
    executable(&messages).ok_or(CargoFailed::NoExecutable)
}

/// The executable the last compiler-artifact message names, if any.
#[must_use]
pub(crate) fn executable(messages: &str) -> Option<PathBuf> {
    messages
        .lines()
        .filter_map(|line| vun_data::json::parse(line).ok())
        .filter(|m| m["reason"].as_str() == Some("compiler-artifact"))
        .filter_map(|m| m["executable"].as_str().map(PathBuf::from))
        .next_back()
}

/// What `cargo <args>` writes to its output, run in the workspace at `root`; its errors go to
/// ours.
///
/// # Errors
/// Returns why cargo cannot run or fails.
pub(crate) fn output(root: &Path, args: &[&'static str]) -> Result<String, CargoFailed> {
    let cargo =
        Program::Cargo.path().ok_or_else(|| CargoFailed::Unstarted(io::ErrorKind::NotFound.into()))?;
    let output = Command::new(cargo)
        .current_dir(root)
        .args(args)
        .stderr(Stdio::inherit())
        .output()
        .map_err(CargoFailed::Unstarted)?;
    if !output.status.success() {
        return Err(CargoFailed::Failed { args: args.to_vec(), status: output.status });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
