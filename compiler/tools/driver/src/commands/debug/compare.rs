//! `vun debug compare`: a program built and run through every backend that generates code, each
//! one's exit status and output, and whether they agree.

use std::io::{self, Write};
use std::path::Path;

use vun_command::{Args, Outcome};
use vun_term::Shell;

use super::FILE;
use crate::Session;
use crate::compare::Ran;

pub(super) fn compare(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let out = &mut shell.out;
    let path = args.value(&FILE).expect("positional parameters are present when a command runs");
    let Some(file) = session.load(Path::new(path)) else { return Ok(session.outcome()) };
    let Ok(runs) = session.run_each(file) else { return Ok(session.outcome()) };
    for run in &runs {
        let status = run.output.status.code().map_or_else(|| "a signal".to_owned(), |c| c.to_string());
        writeln!(out, "{}: exit {status}, {} bytes of output", run.backend, run.output.stdout.len())?;
    }
    let agree = Ran::agree(&runs);
    writeln!(out, "{}", if agree { "the backends agree" } else { "the backends disagree" })?;
    Ok(if agree { Outcome::Success } else { Outcome::Failure })
}
