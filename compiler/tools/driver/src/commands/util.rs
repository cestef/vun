//! `vun util`: infrequently used commands.

use std::io::{self, Write};

use vun_command::{Args, Command, Dialect, Example, Outcome, Param, Values};
use vun_term::Shell;

use super::PROGRAM;
use crate::Session;

const SHELL: Param = Param::positional("shell")
    .about("the shell to complete in")
    .values(Values::Choice(|| Dialect::ALL.iter().map(|d| d.name()).collect()));

pub(super) const COMPLETION: Command<Session> =
    Command::run("completion", "print the completion script of a shell", completion)
        .params(&[SHELL])
        .examples(&[
            Example { line: "bash >> ~/.bashrc", about: "complete in bash" },
            Example {
                line: "nu | save -f ($nu.default-config-dir | path join vun.nu)",
                about: "complete in nushell, then `source` it",
            },
        ]);

fn completion(_: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let dialect = args.choice(&SHELL, &Dialect::ALL, |d| d.name()).expect("parsing checks the shell");
    shell.out.write_all(PROGRAM.completion(*dialect).as_bytes())?;
    Ok(Outcome::Success)
}
