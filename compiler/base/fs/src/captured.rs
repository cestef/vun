//! What a command wrote, read once it has ended.

use std::fs::File;
use std::io;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use super::Scratch;

/// How a command run for a limited time ended.
#[derive(Debug)]
pub enum Ended {
    /// On its own, within the limit.
    Exited(Output),
    /// It was killed at the limit, with what it had written.
    TimedOut(Output),
}

/// How often a command held to a limit is asked whether it has ended.
const POLL: Duration = Duration::from_millis(2);

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

impl Captured {
    /// [`Captured::run`] for at most `limit`: a command still running then is killed.
    ///
    /// # Errors
    /// Returns an error if the command cannot start, or the files cannot be made or read.
    pub fn within(command: &mut Command, limit: Duration) -> io::Result<Ended> {
        let files = Scratch::create("captured")?;
        let (out, err) = (files.join("out"), files.join("err"));
        let mut child =
            command.stdout(File::create(&out)?).stderr(File::create(&err)?).stdin(Stdio::null()).spawn()?;
        let started = Instant::now();
        let (status, timed_out) = loop {
            if let Some(status) = child.try_wait()? {
                break (status, false);
            }
            if started.elapsed() >= limit {
                child.kill()?;
                break (child.wait()?, true);
            }
            std::thread::sleep(POLL);
        };
        let output = Output { status, stdout: std::fs::read(out)?, stderr: std::fs::read(err)? };
        Ok(if timed_out { Ended::TimedOut(output) } else { Ended::Exited(output) })
    }
}
