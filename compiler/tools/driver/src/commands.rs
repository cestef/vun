//! The `vun` command tree (spec 12, section 7), shared by every front end of the driver.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;

use vun_command::{
    Args, Arity, Command, Context, Example, OptionSet, Outcome, Param, PathKind, Program, Values,
};
use vun_span::FileId;
use vun_term::Shell;

use status::{Started, Step};

use crate::{Session, VERSION, codegen, formats, target};

pub const PROGRAM: Program<Session> = Program {
    version: VERSION,
    configure,
    root: Command::group(
        "vun",
        "The Vun toolchain.",
        &[
            Command::run("build", "build the program a root file exports", build)
                .aliases(&["b"])
                .params(&[FILE, crate::codegen::BACKEND, crate::program::OUTPUT])
                .sets(&[COMPILE])
                .examples(&[
                    Example { line: "main.vun", about: "build the program, named after its file" },
                    Example {
                        line: "main.vun -o hello --target linux.x86_64",
                        about: "cross-compile to a path",
                    },
                ]),
            Command::run("run", "build the program a root file exports and run it", run)
                .aliases(&["r"])
                .params(&[FILE, ARGS, crate::codegen::BACKEND, crate::program::OUTPUT])
                .sets(&[COMPILE])
                .examples(&[Example {
                    line: "main.vun -- --verbose input.txt",
                    about: "pass arguments to the program",
                }]),
            CHECK,
            explain::EXPLAIN,
            Command::group(pkg::GROUP, "dependencies and releases", pkg::COMMANDS),
            Command::group("tool", "bindings to other languages", &[tool::BIND]),
            Command::group(toolchain::GROUP, "this toolchain's backends and linkers", toolchain::COMMANDS),
            Command::group(cache::GROUP, "the shared store of fetched packages", cache::COMMANDS),
            Command::group("util", "infrequently used commands", &[util::COMPLETION]),
            Command::group(debug::GROUP, "low-level commands for developing the compiler", debug::COMMANDS)
                .sets(&[COMPILE]),
        ],
    ),
};

const CHECK: Command<Session> = Command::run("check", "analyze without generating code", check)
    .aliases(&["c"])
    .params(&[FILE])
    .sets(&[COMPILE]);
/// Where `vun check` is below the root, for messages that suggest it.
pub(crate) const CHECK_PATH: [&str; 1] = [CHECK.name];

/// The options of every command that compiles a file.
pub(crate) const COMPILE: OptionSet = OptionSet {
    title: "Compile options",
    params: &[target::TARGET, codegen::RELEASE, formats::DIAGNOSTICS, TIMINGS],
};

pub(crate) const FILE: Param =
    Param::positional("file").about("the root source file").values(Values::Path(PathKind::File));
const ARGS: Param = Param::positional("args").arity(Arity::Many).about("arguments passed to the program");
/// Reports at exit where the time went, as `VUN_TIMINGS=1` does.
const TIMINGS: Param = Param::flag("timings").about("report the time of each stage and query at exit");

/// Applies the options of the command.
fn configure(session: &mut Session, args: &Args) -> Result<(), Box<vun_diag::Diagnostic>> {
    if args.flag(&TIMINGS) {
        vun_trace::Timings::enable();
    }
    formats::configure(session, args)?;
    codegen::configure(session, args);
    target::configure(session, args)
}

impl Session {
    /// Reads the file `args` names, once its project is ready: dependencies fetched and the
    /// lockfile written, as cargo does before compiling.
    fn root(&mut self, args: &Args, shell: &mut Shell<'_>) -> Option<FileId> {
        let path =
            Path::new(args.value(&FILE).expect("positional parameters are present when a command runs"));
        for (name, hash, fetched) in self.prepare(path) {
            fetched.report(name, hash, shell);
        }
        if self.failed() {
            return None;
        }
        self.load(path)
    }

    /// Builds the program `args` names, reporting how it went.
    fn built(&mut self, args: &Args, shell: &mut Shell<'_>) -> Option<PathBuf> {
        let started = Instant::now();
        let path = args.value(&FILE).expect("positional parameters are present when a command runs");
        shell.status(&Started { step: Step::Build, subject: path.to_owned() });
        let output = args.value(&crate::program::OUTPUT).map(Path::new);
        let built = self.root(args, shell).and_then(|file| self.build(file, output).ok());
        let subject = built.as_ref().map_or_else(|| path.to_owned(), |p| p.display().to_string());
        self.conclude(shell, subject, started);
        built
    }

    /// How a command that reported its failures as diagnostics ends.
    #[must_use]
    pub fn outcome(&self) -> Outcome {
        if self.failed() { Outcome::Failure } else { Outcome::Success }
    }
}

#[expect(clippy::unnecessary_wraps, reason = "every command has the `Run` signature")]
fn build(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    session.built(args, shell);
    Ok(session.outcome())
}

fn run(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let Some(program) = session.built(args, shell) else { return Ok(session.outcome()) };
    shell.status(&Started { step: Step::Run, subject: program.display().to_string() });
    let status = std::process::Command::new(&program).args(args.values(&ARGS)).status()?;
    // A program ended by a signal has no status to pass on.
    Ok(status.code().and_then(|c| u8::try_from(c).ok()).map_or(Outcome::Failure, Outcome::Exited))
}

#[expect(clippy::unnecessary_wraps, reason = "every command has the `Run` signature")]
fn check(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let started = Instant::now();
    let path = args.value(&FILE).expect("positional parameters are present when a command runs");
    shell.status(&Started { step: Step::Check, subject: path.to_owned() });
    if let Some(file) = session.root(args, shell) {
        session.check(file);
    }
    session.conclude(shell, path.to_owned(), started);
    Ok(session.outcome())
}

impl Context for Session {
    fn refused(&mut self, diagnostic: vun_diag::Diagnostic) {
        self.diag.error(diagnostic);
    }

    fn finish(&mut self, shell: &mut Shell<'_>) {
        self.emit(shell);
        self.save();
        if let Some(report) = vun_trace::Timings::report() {
            shell.status(&report);
        }
    }
}

mod cache;
mod debug;
mod explain;
mod pkg;
mod status;
mod tool;
mod toolchain;
mod util;

#[cfg(test)]
mod tests;
