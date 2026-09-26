//! `cargo xtask size`: the distributed `vun` stays within its budget (decision 131). The budget
//! is 12 MiB (decision 193), raised from 10 MiB (decision 139) once the native backend took the
//! build to 11,172,560 bytes on `x86_64-unknown-linux-gnu`; raising it takes a decision, as a new
//! external crate does.

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The most bytes the stripped `dist` build of `vun` may take on the host.
pub(crate) const BUDGET: u64 = 12 * 1024 * 1024;

/// The package whose binary is measured.
const PACKAGE: &str = "vun-cli";
/// The profile what is distributed is built with (`[profile.dist]`).
const PROFILE: &str = "dist";

/// The measured binary.
pub(crate) struct Measured {
    pub(crate) path: PathBuf,
    pub(crate) bytes: u64,
}

impl Measured {
    /// Builds `vun` as it is distributed from the workspace at `root` and measures it.
    ///
    /// # Errors
    /// Returns an error if cargo cannot run or fails, or names no executable.
    pub(crate) fn build(root: &Path) -> io::Result<Self> {
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let output = Command::new(cargo)
            .current_dir(root)
            .args(["build", "--quiet", "--message-format=json", "--profile", PROFILE, "--package", PACKAGE])
            .stderr(Stdio::inherit())
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other(format!("cargo build --profile {PROFILE} failed")));
        }
        let messages = String::from_utf8_lossy(&output.stdout);
        let path = Self::executable(&messages)
            .ok_or_else(|| io::Error::other(format!("cargo built no executable of {PACKAGE}")))?;
        let bytes = std::fs::metadata(&path)?.len();
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
