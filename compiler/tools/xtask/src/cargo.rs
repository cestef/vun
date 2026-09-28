//! Running cargo on the workspace, for the checks that read what it reports.

use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::{fmt, io};

/// Why cargo reported nothing.
#[derive(Debug)]
pub(crate) enum CargoFailed {
    Unstarted(io::Error),
    Failed { args: Vec<&'static str>, status: ExitStatus },
}

impl fmt::Display for CargoFailed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unstarted(_) => f.write_str("cargo cannot run"),
            Self::Failed { args, status } => write!(f, "`cargo {}` failed: {status}", args.join(" ")),
        }
    }
}

impl std::error::Error for CargoFailed {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unstarted(e) => Some(e),
            Self::Failed { .. } => None,
        }
    }
}

/// What `cargo <args>` writes to its output, run in the workspace at `root`; its errors go to
/// ours.
///
/// # Errors
/// Returns why cargo cannot run or fails.
pub(crate) fn output(root: &Path, args: &[&'static str]) -> Result<String, CargoFailed> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
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
