//! External tools: assemblers, C compilers, build steps.
//!
//! Anything the toolchain runs as a separate program goes through [`Tool`], with declared inputs
//! and outputs so the build can cache and sandbox it (spec 12, section 8).

use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Command;

use vun_intern::Symbol;

pub mod gcc;

/// An argument of a tool: text, or a path, passed as the system spells it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Arg {
    Text(Symbol),
    Path(PathBuf),
}

impl Arg {
    /// The argument as the system passes it.
    #[must_use]
    pub fn os(&self) -> &OsStr {
        match self {
            Self::Text(text) => OsStr::new(text.as_str()),
            Self::Path(path) => path.as_os_str(),
        }
    }
}

impl From<&'static str> for Arg {
    fn from(text: &'static str) -> Self {
        Self::Text(Symbol::intern(text))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Invocation {
    pub args: Vec<Arg>,
    /// Environment variables set for the run; nothing else is inherited when sandboxed.
    pub env: Vec<(Symbol, Arg)>,
    pub cwd: Option<PathBuf>,
    /// Files the run reads. Part of the cache key.
    pub inputs: Vec<PathBuf>,
    /// Files the run writes.
    pub outputs: Vec<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Output {
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl Output {
    #[must_use]
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

/// Why a run of a tool did not succeed.
#[derive(Debug)]
pub enum Failure {
    /// It could not be started.
    Start(std::io::Error),
    /// It ended with this status, `None` when a signal ended it, having printed this.
    Exit { code: Option<i32>, stderr: String },
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Start(_) => f.write_str("it could not be started"),
            Self::Exit { code: Some(code), stderr } => write!(f, "it exited with status {code}:\n{stderr}"),
            Self::Exit { code: None, stderr } => write!(f, "a signal ended it:\n{stderr}"),
        }
    }
}

impl std::error::Error for Failure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Start(e) => Some(e),
            Self::Exit { .. } => None,
        }
    }
}

pub trait Tool {
    /// Stable identifier, such as `cc` or `qbe`.
    fn id(&self) -> &str;

    /// # Errors
    /// Returns an error if the tool could not be started.
    fn run(&self, inv: &Invocation) -> std::io::Result<Output>;

    /// Runs, and fails unless the run succeeds.
    ///
    /// # Errors
    /// Returns why the run did not succeed.
    fn check(&self, inv: &Invocation) -> Result<Output, Failure> {
        let out = self.run(inv).map_err(Failure::Start)?;
        if out.success() {
            return Ok(out);
        }
        Err(Failure::Exit { code: out.code, stderr: String::from_utf8_lossy(&out.stderr).into_owned() })
    }
}

/// A tool found on the system as an executable.
pub struct Executable {
    id: String,
    program: PathBuf,
}

impl Executable {
    pub fn new(id: impl Into<String>, program: impl Into<PathBuf>) -> Self {
        Self { id: id.into(), program: program.into() }
    }

    /// Where the program `name` is on the search path (`PATH`), if it is.
    #[must_use]
    pub fn search(name: &str) -> Option<PathBuf> {
        let paths = std::env::var_os("PATH")?;
        std::env::split_paths(&paths).map(|dir| dir.join(name)).find(|path| path.is_file())
    }
}

impl Tool for Executable {
    fn id(&self) -> &str {
        &self.id
    }

    fn run(&self, inv: &Invocation) -> std::io::Result<Output> {
        let mut cmd = Command::new(&self.program);
        cmd.args(inv.args.iter().map(Arg::os));
        cmd.envs(inv.env.iter().map(|(k, v)| (k.as_str(), v.os())));
        if let Some(cwd) = &inv.cwd {
            cmd.current_dir(cwd);
        }
        let out = cmd.output()?;
        Ok(Output { code: out.status.code(), stdout: out.stdout, stderr: out.stderr })
    }
}
