//! Profile-guided optimization of `vun` (decision 670): a `vun` built to record which of its
//! branches and calls run is run over the workloads `vun debug profile` measures, and what it
//! records is merged into the profile rustc lays out the distributed `vun` by. The recording
//! build links the components in, so the workloads run without their libraries beside it; the
//! crates it shares with the distributed `vun` are the compiler's, which is what the profile is
//! for. A machine records the profile of its own `vun`, as the recording one runs there.

use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};

use vun_command::{Args, Outcome, Shell};
use vun_diag::{DiagCtxt, DiagId, Diagnostic, Msg};
use vun_fs::Edit;
use vun_intern::Symbol;
use vun_term::{Record, Status, Style, Text, Verb};
use vun_tool::{Arg, Executable, Failure, Invocation, Streams, Tool};

use crate::cargo::{self, CargoFailed, Flavor};
use crate::task::Program;

/// The workloads of `vun debug profile` the recording `vun` runs: checking small programs, the
/// standard library and compile-time evaluation, building programs, and running one.
const WORKLOADS: &[&str] =
    &["check-hello", "check-tour", "check-comptime", "check-std", "build-hello", "build-tour", "run-fib"];

/// The profile the recording `vun` is built with: optimized as the distributed one is, without
/// its slower whole-program link.
const RECORDING: &str = "release";

/// Where profiles are made, in the workspace.
const DIR: &str = "target/pgo";
/// The recording build's own target directory, apart from the workspace's.
const BUILT: &str = "build";
const RECORDS: &str = "records";
const PROFILE: &str = "vun.profdata";

/// The variable an instrumented program reads where to write what it records from, and the
/// file each writes: one per program and process (`%m`, `%p`).
const RECORD_FILE: &str = "LLVM_PROFILE_FILE";
const RECORD_NAME: &str = "vun-%m-%p.profraw";

const UNPROFILED: DiagId = DiagId::new("xtask.pgo.unprofiled");

/// Why no profile was made.
#[derive(Debug)]
pub(crate) enum PgoFailed {
    /// The recording `vun` did not build.
    Built(CargoFailed),
    /// Its records could not be made room for.
    Records(std::io::Error),
    /// The tool merging records is not installed (`rustup component add llvm-tools`).
    NoMerger,
    /// The recording `vun` failed its workloads.
    Recorded(Failure),
    /// The records did not merge.
    Merged(Failure),
}

impl fmt::Display for PgoFailed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Built(_) => "the recording `vun` does not build",
            Self::Records(_) => "the directory of the records cannot be made",
            Self::NoMerger => "`llvm-profdata` is not installed: `rustup component add llvm-tools`",
            Self::Recorded(_) => "the recording `vun` fails its workloads",
            Self::Merged(_) => "the records do not merge",
        })
    }
}

impl std::error::Error for PgoFailed {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Built(e) => Some(e),
            Self::Records(e) => Some(e),
            Self::Recorded(e) | Self::Merged(e) => Some(e),
            Self::NoMerger => None,
        }
    }
}

impl PgoFailed {
    pub(crate) fn diagnostic(&self) -> Diagnostic {
        let msg = Msg::new().text("cannot make the profile ").code("vun").text(" is optimized by");
        Diagnostic::error(UNPROFILED, msg).caused_by(self)
    }
}

/// The profile of the workspace at `root`, made in its target directory.
pub(crate) struct Pgo {
    root: PathBuf,
    dir: PathBuf,
}

impl Pgo {
    pub(crate) fn of(root: &Path) -> Self {
        Self { root: root.to_owned(), dir: root.join(DIR) }
    }

    /// Builds the recording `vun`, runs the workloads with it and merges what it records,
    /// returning the profile.
    ///
    /// # Errors
    /// Returns which step failed, and why.
    pub(crate) fn make(&self) -> Result<PathBuf, PgoFailed> {
        let merger = Program::LlvmProfdata.path().ok_or(PgoFailed::NoMerger)?;
        let records = self.dir.join(RECORDS);
        records.fresh().map_err(PgoFailed::Records)?;
        let flavor = Flavor {
            linked: true,
            rustflags: vec![flag("-Cprofile-generate=", &records)],
            target_dir: Some(self.dir.join(BUILT)),
        };
        let recording = cargo::vun_as(&self.root, RECORDING, &flavor).map_err(PgoFailed::Built)?;
        let mut args = vec![Arg::from("debug"), Arg::from("profile")];
        args.extend(WORKLOADS.iter().map(|&w| Arg::from(w)));
        args.extend(["--measure", "time", "--samples", "1", "--exe"].map(Arg::from));
        args.push(Arg::Path(recording.clone()));
        let run = Invocation {
            args,
            env: vec![(Symbol::intern(RECORD_FILE), Arg::Path(records.join(RECORD_NAME)))],
            cwd: Some(self.root.clone()),
            streams: Streams::Inherited,
            ..Invocation::default()
        };
        Executable::new("vun", &recording).check(&run).map_err(PgoFailed::Recorded)?;
        let profile = self.dir.join(PROFILE);
        let merge = Invocation {
            args: vec![Arg::from("merge"), Arg::from("-o"), Arg::Path(profile.clone()), Arg::Path(records)],
            ..Invocation::default()
        };
        Executable::new(Program::LlvmProfdata.name(), merger).check(&merge).map_err(PgoFailed::Merged)?;
        Ok(profile)
    }

    /// The flags rustc lays out a `vun` by `profile` with; a function the profile does not know,
    /// as one of a component or a feature the recording `vun` lacked, is laid out as without.
    pub(crate) fn flags(profile: &Path) -> Vec<OsString> {
        vec![flag("-Cprofile-use=", profile), OsString::from("-Cllvm-args=-pgo-warn-mismatch=false")]
    }
}

fn flag(name: &str, path: &Path) -> OsString {
    let mut flag = OsString::from(name);
    flag.push(path);
    flag
}

/// `cargo xtask pgo`: makes the profile, for a build to use with [`Pgo::flags`].
#[expect(clippy::unnecessary_wraps, reason = "every command has the `Run` signature")]
pub(crate) fn pgo(diag: &mut DiagCtxt, _: &Args, shell: &mut Shell<'_>) -> std::io::Result<Outcome> {
    match Pgo::of(&vun_harness::repo_root()).make() {
        Ok(profile) => {
            shell.status(&Profiled { profile });
            Ok(Outcome::Success)
        }
        Err(why) => {
            diag.error(why.diagnostic());
            Ok(Outcome::Failure)
        }
    }
}

/// The profile made.
struct Profiled {
    profile: PathBuf,
}

impl Record for Profiled {
    fn kind(&self) -> &'static str {
        "profiled"
    }

    fn text(&self) -> Text {
        Text::of(Style::Dim, self.profile.display().to_string())
    }

    fn fields(&self) -> vun_data::Object<'static> {
        vun_data::Object::new().with("profile", self.profile.display().to_string())
    }
}

impl Status for Profiled {
    fn verb(&self) -> Verb {
        Verb::good("Profiled")
    }
}
