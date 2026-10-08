//! Running cargo on the workspace, for the checks that read what it reports.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::{fmt, io};

use vun_tool::{Arg, Invocation, Tool};

use crate::task::Program;

/// The package whose `vun` executable a build makes.
const PACKAGE: &str = "vun-cli";

/// The profile what is distributed is built with (`[profile.dist]`): its `vun` links only what
/// the feature [`DIST`] of the package names, the rest being components it loads.
pub(crate) const DISTRIBUTED: &str = "dist";
const DIST: &str = "dist";

/// The variable the driver reads its build from.
const BUILD: &str = "VUN_BUILD";
/// The variables cargo reads rustc's flags from: separated by spaces, and by unit separators,
/// which the second takes before the first.
const RUSTFLAGS: &str = "RUSTFLAGS";
const ENCODED_RUSTFLAGS: &str = "CARGO_ENCODED_RUSTFLAGS";
const SEPARATOR: &str = "\x1f";
const TARGET_DIR: &str = "CARGO_TARGET_DIR";

/// How a `vun` is built besides its profile.
#[derive(Default)]
pub(crate) struct Flavor {
    /// With the package's default features, which link the components in, whatever the profile.
    pub(crate) linked: bool,
    /// Flags rustc takes after those of the environment.
    pub(crate) rustflags: Vec<OsString>,
    /// Where cargo builds, when not in the workspace's target directory.
    pub(crate) target_dir: Option<PathBuf>,
}

impl Flavor {
    /// The variables cargo runs with.
    fn env(&self) -> Vec<(&'static str, OsString)> {
        let mut env: Vec<(&str, OsString)> = self.target_dir.iter().map(|d| (TARGET_DIR, d.into())).collect();
        if !self.rustflags.is_empty() {
            let outer = std::env::var(RUSTFLAGS).unwrap_or_default();
            let flags = outer.split_whitespace().map(OsString::from).chain(self.rustflags.iter().cloned());
            env.push((ENCODED_RUSTFLAGS, flags.collect::<Vec<_>>().join(std::ffi::OsStr::new(SEPARATOR))));
        }
        env
    }
}

/// Why cargo reported nothing.
#[derive(Debug)]
pub(crate) enum CargoFailed {
    Unstarted(io::Error),
    Failed {
        args: Vec<String>,
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
pub(crate) fn vun(root: &Path, profile: &str) -> Result<PathBuf, CargoFailed> {
    vun_as(root, profile, &Flavor::default())
}

/// [`vun`], built as `flavor` says.
///
/// # Errors
/// As [`vun`].
pub(crate) fn vun_as(root: &Path, profile: &str, flavor: &Flavor) -> Result<PathBuf, CargoFailed> {
    let mut args = ["build", "--message-format=json", "--profile", profile, "--package", PACKAGE]
        .map(String::from)
        .to_vec();
    if profile == DISTRIBUTED && !flavor.linked {
        args.extend(["--no-default-features", "--features", DIST].map(String::from));
    }
    let mut env = flavor.env();
    env.extend(Build::of(root).map(|id| (BUILD, id.into())));
    let messages = run(root, &args, &env)?;
    executable(&messages).ok_or(CargoFailed::NoExecutable)
}

/// Builds the libraries of the component crates `packages` in `profile`, as `tool` does (`build`,
/// or `zigbuild`) for `target`, beside the `vun` of that profile.
///
/// # Errors
/// Returns why cargo cannot run or fails.
pub(crate) fn components(
    root: &Path,
    profile: &str,
    tool: &str,
    target: Option<&str>,
    packages: &[String],
) -> Result<(), CargoFailed> {
    let mut args = vec![tool.to_owned(), "--profile".to_owned(), profile.to_owned()];
    args.extend(target.iter().flat_map(|t| ["--target".to_owned(), (*t).to_owned()]));
    for package in packages {
        args.extend([
            "--package".to_owned(),
            package.clone(),
            "--features".to_owned(),
            format!("{package}/export"),
        ]);
    }
    run(root, &args, &[]).map(drop)
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
pub(crate) fn output(root: &Path, args: &[&str]) -> Result<String, CargoFailed> {
    run(root, &args.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>(), &[])
}

/// What `cargo metadata` says of the workspace at `root`, its crates alone.
///
/// # Errors
/// Returns why cargo cannot run or fails.
pub(crate) fn metadata(root: &Path) -> Result<String, CargoFailed> {
    output(root, &["metadata", "--no-deps", "--format-version", "1"])
}

/// The commit the checkout is at, which `vun --version` reports.
struct Build;

impl Build {
    /// The commit of the working copy at `root`, none where `jj` is absent or `root` is no
    /// repository (a mirror on a build host).
    fn of(root: &Path) -> Option<String> {
        let inv = Invocation {
            args: ["log", "--no-graph", "--ignore-working-copy", "-r", "@", "-T", "commit_id.short(12)"]
                .map(Arg::from)
                .to_vec(),
            cwd: Some(root.to_owned()),
            ..Invocation::default()
        };
        let out = Program::Jj.find().ok()?.check(&inv).ok()?;
        Some(String::from_utf8_lossy(&out.stdout).trim().to_owned()).filter(|id| !id.is_empty())
    }
}

/// [`output`], with the variables `env` set: the commit the build is of, for the driver to
/// report, and how it is built.
fn run(root: &Path, args: &[String], env: &[(&str, OsString)]) -> Result<String, CargoFailed> {
    let cargo =
        Program::Cargo.path().ok_or_else(|| CargoFailed::Unstarted(io::ErrorKind::NotFound.into()))?;
    let output = Command::new(cargo)
        .current_dir(root)
        .args(args)
        .envs(env.iter().map(|(name, value)| (name, value)))
        .stderr(Stdio::inherit())
        .output()
        .map_err(CargoFailed::Unstarted)?;
    if !output.status.success() {
        return Err(CargoFailed::Failed { args: args.to_vec(), status: output.status });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
