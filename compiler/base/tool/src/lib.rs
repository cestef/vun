//! External tools: assemblers, C compilers, build steps.
//!
//! Anything the toolchain runs as a separate program goes through [`Tool`], with declared inputs
//! and outputs so the build can cache and sandbox it (spec 12, section 8).

use std::path::PathBuf;
use std::process::Command;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Invocation {
    pub args: Vec<String>,
    /// Environment variables set for the run; nothing else is inherited when sandboxed.
    pub env: Vec<(String, String)>,
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

pub trait Tool {
    /// Stable identifier, such as `cc` or `qbe`.
    fn id(&self) -> &str;

    /// # Errors
    /// Returns an error if the tool could not be started.
    fn run(&self, inv: &Invocation) -> std::io::Result<Output>;
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
}

impl Tool for Executable {
    fn id(&self) -> &str {
        &self.id
    }

    fn run(&self, inv: &Invocation) -> std::io::Result<Output> {
        let mut cmd = Command::new(&self.program);
        cmd.args(&inv.args);
        cmd.envs(inv.env.iter().map(|(k, v)| (k, v)));
        if let Some(cwd) = &inv.cwd {
            cmd.current_dir(cwd);
        }
        let out = cmd.output()?;
        Ok(Output { code: out.status.code(), stdout: out.stdout, stderr: out.stderr })
    }
}
