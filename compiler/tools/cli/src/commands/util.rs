//! `vun util`: infrequently used commands.

use std::io;

use vun_command::{Args, Command, Dialect, Example, Outcome, Param, Values};
use vun_diag::{Diagnostic, Msg, errors};
use vun_term::{Document, Shell};

use super::PROGRAM;
use vun_driver::Session;

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

pub(super) const LSP: Command<Session> =
    Command::run("lsp", "serve the language server over standard input and output", lsp);

/// Runs the language server the program installed (spec 13, section 11).
fn lsp(session: &mut Session, _: &Args, _: &mut Shell<'_>) -> io::Result<Outcome> {
    if let Some(serve) = session.language_server {
        return serve(session);
    }
    let msg = Msg::new().text("this program runs without ").code("util lsp");
    session.diag.error(Diagnostic::error(errors::CLI_UNAVAILABLE, msg));
    Ok(Outcome::Failure)
}

fn completion(_: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let dialect = args.choice(&SHELL, &Dialect::ALL, |d| d.name()).expect("parsing checks the shell");
    let script =
        Document { kind: "completion", name: dialect.name().to_owned(), text: PROGRAM.completion(*dialect) };
    shell.result(&script)?;
    Ok(Outcome::Success)
}
