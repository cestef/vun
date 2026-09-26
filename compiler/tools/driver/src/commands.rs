//! The `vun` command tree (spec 12, section 7), shared by every front end of the driver.

use std::io::{self, Write};
use std::path::Path;

use vun_command::{Action, Args, Command, Outcome, Param, ParamKind, Program};

use crate::{Session, VERSION, formats, target};

pub const PROGRAM: Program<Session> = Program {
    version: VERSION,
    configure,
    root: Command {
        name: "vun",
        about: "The Vun toolchain.",
        params: &[formats::FORMAT, target::TARGET, TIMINGS],
        action: Action::Group(&[
            Command::planned("new", "create a package"),
            Command {
                name: "build",
                about: "build the program a root file exports",
                params: &[FILE, crate::codegen::BACKEND, crate::program::OUTPUT],
                action: Action::Run(build),
            },
            Command {
                name: "run",
                about: "build the program a root file exports and run it",
                params: &[FILE, crate::codegen::BACKEND, crate::program::OUTPUT],
                action: Action::Run(run),
            },
            Command {
                name: "check",
                about: "analyze without generating code",
                params: &[FILE],
                action: Action::Run(check),
            },
            Command::planned("test", "run tests, properties and simulations"),
            Command::planned("bench", "run benchmarks"),
            Command::planned("fuzz", "fuzz a test with coverage guidance"),
            Command::planned("fmt", "format source files"),
            Command::planned("fix", "apply fixes from diagnostics"),
            Command::planned("doc", "generate documentation"),
            explain::EXPLAIN,
            Command::group(
                pkg::GROUP,
                "dependencies, releases and API diffs",
                &[
                    Command {
                        name: "add",
                        about: "declare a dependency, fetching an archive to learn its hash",
                        params: &[pkg::SOURCE, pkg::NAME, pkg::VERSION],
                        action: Action::Run(pkg::add),
                    },
                    Command {
                        name: "update",
                        about: "raise dependencies to the newest release their indexes list",
                        params: &[],
                        action: Action::Run(pkg::update),
                    },
                    pkg::FETCH,
                    Command::planned("vendor", "copy dependencies into vendor/"),
                    Command {
                        name: "pack",
                        about: "write the package's archive",
                        params: &[pkg::OUT],
                        action: Action::Run(pkg::pack),
                    },
                    Command::planned("why", "explain why a package is in the graph"),
                    Command::planned("tag", "release workspace members"),
                    Command::planned("api", "print the public API, or diff it against a release"),
                ],
            ),
            Command::group(
                "tool",
                "C bindings, C headers, register packages",
                &[
                    tool::BIND,
                    Command::planned("header", "emit a C header for a library"),
                    Command::planned("svd", "generate a register package from an SVD file"),
                ],
            ),
            Command::group(
                "target",
                "flash, attach to and debug devices",
                &[
                    Command::planned("flash", "write a program to a device"),
                    Command::planned("attach", "stream logs from a device"),
                    Command::planned("debug", "run a GDB server for a device"),
                ],
            ),
            Command::group(
                toolchain::GROUP,
                "installed toolchains, and this one's backends and linkers",
                toolchain::COMMANDS,
            ),
            Command::group(cache::GROUP, "the shared store of fetched packages", cache::COMMANDS),
            Command::group(
                "util",
                "infrequently used commands",
                &[
                    Command::planned("lsp", "run the language server"),
                    Command::planned("completion", "print a completion script for a shell"),
                ],
            ),
            Command::group(debug::GROUP, "low-level commands for developing the compiler", debug::COMMANDS),
        ]),
    },
};

/// Builds the program `args` names, or reports why it cannot.
fn built(session: &mut Session, args: &Args) -> Option<std::path::PathBuf> {
    let path = args.value(&FILE).expect("positional parameters are present when a command runs");
    let file = session.load(Path::new(path))?;
    let backend = args.value(&crate::codegen::BACKEND);
    session.build(file, backend, args.value(&crate::program::OUTPUT).map(Path::new)).ok()
}

#[expect(clippy::unnecessary_wraps, reason = "every command has the `Run` signature")]
fn build(session: &mut Session, args: &Args, _: &mut dyn Write) -> io::Result<Outcome> {
    built(session, args);
    Ok(session.outcome())
}

fn run(session: &mut Session, args: &Args, _: &mut dyn Write) -> io::Result<Outcome> {
    let Some(program) = built(session, args) else { return Ok(session.outcome()) };
    let status = std::process::Command::new(&program).status()?;
    // A program ended by a signal has no status to pass on.
    Ok(status.code().and_then(|c| u8::try_from(c).ok()).map_or(Outcome::Failure, Outcome::Exited))
}

/// Reports at exit where the time went, as `VUN_TIMINGS=1` does.
const TIMINGS: Param = Param {
    name: "timings",
    short: None,
    about: "report the time of each stage and query at exit",
    kind: ParamKind::Flag,
};

/// Applies the options every command takes.
fn configure(session: &mut Session, args: &Args) -> Result<(), Box<vun_diag::Diagnostic>> {
    if args.flag(&TIMINGS) {
        vun_trace::Timings::enable();
    }
    formats::configure(session, args)?;
    target::configure(session, args)
}

pub(crate) const FILE: Param =
    Param { name: "file", short: None, about: "the source file to read", kind: ParamKind::Positional };

#[expect(clippy::unnecessary_wraps, reason = "every command has the `Run` signature")]
fn check(session: &mut Session, args: &Args, _: &mut dyn Write) -> io::Result<Outcome> {
    let path = args.value(&FILE).expect("positional parameters are present when a command runs");
    if let Some(file) = session.load(Path::new(path)) {
        session.check(file);
    }
    Ok(session.outcome())
}

mod cache;
mod debug;
mod explain;
mod pkg;
mod tool;
mod toolchain;

pub(crate) use pkg::FETCH_PATH;
#[cfg(test)]
mod tests;

impl Session {
    /// How a command that reported its failures as diagnostics ends.
    #[must_use]
    pub fn outcome(&self) -> Outcome {
        if self.failed() { Outcome::Failure } else { Outcome::Success }
    }
}
