//! What a command wrote, read once it has ended.

use std::fs::File;
use std::io;
use std::process::{Command, Output, Stdio};

use super::Scratch;

/// A command run to its end, its output and errors collected in files.
///
/// Pipes are read until every holder closes them, and a command may start a daemon that holds
/// them long after it ends (`wine64` leaves its `wineserver`, which exits seconds later), so the
/// run would last as long as the daemon. A file has no end to wait for.
pub struct Captured;

impl Captured {
    /// Runs `command` and returns what it wrote, ended with its process.
    ///
    /// # Errors
    /// Returns an error if the command cannot start, or the files cannot be made or read.
    pub fn run(command: &mut Command) -> io::Result<Output> {
        let files = Scratch::create("captured")?;
        let (out, err) = (files.join("out"), files.join("err"));
        let status =
            command.stdout(File::create(&out)?).stderr(File::create(&err)?).stdin(Stdio::null()).status()?;
        Ok(Output { status, stdout: std::fs::read(out)?, stderr: std::fs::read(err)? })
    }
}
