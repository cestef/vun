//! LLVM's assembler as an oracle for the assemblers' tests: it assembles a text for a processor
//! into an object, or refuses it, which its exit status says.

use std::io::Write;
use std::process::{Command, Stdio};

/// LLVM's assembler, for the processor its arguments select (`--triple`, `-mcpu`, `-mattr`).
#[derive(Clone, Copy, Debug)]
pub struct Assembler {
    pub args: &'static [&'static str],
}

/// What the assembler made of a text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Assembled {
    /// The object file of the text.
    Object(Vec<u8>),
    /// It refused the text.
    Refused,
}

impl Assembler {
    const PROGRAM: &str = "llvm-mc";

    /// What the assembler makes of `text`: `None` where it cannot run.
    #[must_use]
    pub fn assemble(self, text: &str) -> Option<Assembled> {
        let mut child = Command::new(Self::PROGRAM)
            .args(self.args)
            .args(["-filetype=obj", "-o", "-", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        child.stdin.take()?.write_all(text.as_bytes()).ok()?;
        let out = child.wait_with_output().ok()?;
        Some(if out.status.success() { Assembled::Object(out.stdout) } else { Assembled::Refused })
    }
}
