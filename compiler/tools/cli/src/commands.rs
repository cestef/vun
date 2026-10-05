//! The `vun` command tree (spec 12, section 7), shared by every front end of the driver.

use std::io;
use std::path::Path;
use std::time::Instant;

use vun_command::{Args, Arity, Command, Example, OptionSet, Outcome, Param, PathKind, Program, Values};
use vun_term::Shell;

use inputs::{Inputs, Named};
use settings::defaults;
use status::{Conclude, Started, Step};

use vun_driver::subject::Subject;
use vun_driver::{BUILD, Session, VERSION, checks, codegen, cover, formats, trace};

pub(crate) const PROGRAM: Program<Session> = Program {
    version: VERSION,
    build: BUILD,
    defaults,
    configure,
    root: Command::group(
        "vun",
        "The Vun toolchain.",
        &[
            Command::run(
                "build",
                "build the program a root file exports, or the programs each package's plan records",
                build,
            )
            .aliases(&["b"])
            .params(&[
                PATH,
                vun_driver::codegen::BACKEND,
                vun_driver::codegen::LINKER,
                vun_driver::codegen::LIBRARY,
                vun_driver::program::OUTPUT,
            ])
            .sets(&[COMPILE])
            .examples(&[
                Example { line: "main.vun", about: "build the program, named after its file" },
                Example {
                    line: "main.vun -o hello --target linux.x86_64.kernel",
                    about: "cross-compile to a path",
                },
                Example {
                    line: "",
                    about: "carry out the plan of the package around this directory, or of each package of its workspace",
                },
            ]),
            Command::run("run", "build the program a root file exports, or a package's, and run it", run)
                .aliases(&["r"])
                .params(&[
                    PATH,
                    ARGS,
                    vun_driver::codegen::BACKEND,
                    vun_driver::codegen::LINKER,
                    vun_driver::program::OUTPUT,
                ])
                .sets(&[COMPILE])
                .examples(&[
                    Example { line: "", about: "run the program the plan of the package around this directory builds" },
                    Example { line: "main.vun -- --verbose input.txt", about: "pass arguments to the program" },
                ]),
            new::NEW,
            flash::FLASH,
            CHECK,
            fix::FIX,
            doc::DOC,
            explain::EXPLAIN,
            fmt::FMT,
            test::TEST,
            test::BENCH,
            test::FUZZ,
            Command::group(pkg::GROUP, "dependencies and releases", pkg::COMMANDS),
            #[cfg(not(target_os = "wasi"))]
            Command::group(ffi::GROUP, "bindings to and from other languages", &[ffi::BIND]),
            Command::group(target::GROUP, "what builds for each target, and from where", target::COMMANDS),
            Command::group(
                toolchain::GROUP,
                "this toolchain: its version, library and components",
                toolchain::COMMANDS,
            ),
            Command::group(cache::GROUP, "the shared store of fetched packages", cache::COMMANDS),
            Command::group("util", "infrequently used commands", &[util::COMPLETION, util::LSP]),
            Command::group(debug::GROUP, "low-level commands for developing the compiler", debug::COMMANDS)
                .sets(&[COMPILE]),
        ],
    ),
};

const CHECK: Command<Session> = Command::run("check", "analyze without generating code", check)
    .aliases(&["c"])
    .params(&[PATH])
    .sets(&[COMPILE])
    .examples(&[
        Example {
            line: "",
            about: "check every file of the package around this directory, or of its workspace",
        },
        Example { line: "net/", about: "check the files of a directory" },
        Example { line: "main.vun", about: "check a file and what its program reaches" },
    ]);
/// Where `vun check` is below the root, for messages that suggest it.
pub(crate) const CHECK_PATH: [&str; 1] = [CHECK.name];

/// The options of every command that compiles a file.
pub(crate) const COMPILE: OptionSet = OptionSet {
    title: "Compile options",
    params: &[
        vun_driver::target::TARGET,
        checks::CHECKS,
        cover::COVER,
        vun_driver::lints::LINTS,
        trace::TRACE,
        codegen::RELEASE,
        formats::DIAGNOSTICS,
        TIMINGS,
        TIMELINE,
        MEMOS,
    ],
};

pub(crate) const FILE: Param =
    Param::positional("file").about("the root source file").values(Values::Path(PathKind::File));
/// What a command acts on (decision 356).
pub(crate) const PATH: Param = Param::positional("path")
    .arity(Arity::Optional)
    .about("a file, a directory or a package's directory; the package around this directory if none")
    .values(Values::Path(PathKind::Any));
const ARGS: Param = Param::positional("args").arity(Arity::Many).about("arguments passed to the program");
/// Reports at exit where the time went, as `VUN_TIMINGS=1` does.
const TIMINGS: Param = Param::flag("timings")
    .about("report the time of each stage and query, and how work spread over threads, at exit");
/// Writes every run of each stage and query at exit, for Perfetto, as `VUN_TIMELINE` does.
const TIMELINE: Param = Param::option("timeline")
    .about("write every run of each stage and query, on every thread, to a file Perfetto opens, at exit")
    .values(Values::Path(PathKind::File));
/// Reports at exit what the queries did with their memos, as `VUN_MEMOS=1` does.
const MEMOS: Param = Param::flag("memos").about("report what each query did with its memos at exit");

/// Applies the options of the command.
fn configure(session: &mut Session, args: &Args) -> Result<(), Box<vun_diag::Diagnostic>> {
    if args.flag(&TIMINGS) {
        vun_trace::Timings::enable();
    }
    if let Some(path) = args.value(&TIMELINE) {
        vun_trace::Timeline::record_to(Path::new(path));
    }
    session.tally |= args.flag(&MEMOS);
    formats::configure(session, args)?;
    codegen::configure(session, args);
    checks::configure(session, args)?;
    cover::configure(session, args);
    vun_driver::lints::configure(session, args)?;
    trace::configure(session, args);
    vun_driver::target::configure(session, args)
}

fn build(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    session.built(args, shell)?;
    Ok(session.outcome())
}

fn run(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let Some(built) = session.built(args, shell)? else { return Ok(session.outcome()) };
    let Some(program) = session.to_run(&built) else { return Ok(session.outcome()) };
    shell.status(&Started { step: Step::Run, subject: Subject::relative(&program.path) });
    let status = session.command(&program.path).args(args.values(&ARGS)).status()?;
    // A program ended by a signal has no status to pass on.
    Ok(status.code().and_then(|c| u8::try_from(c).ok()).map_or(Outcome::Failure, Outcome::Exited))
}

fn check(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let started = Instant::now();
    let Some(Named { subject, shown, ready }) = session.named(args, Step::Check, shell)? else {
        return Ok(session.outcome());
    };
    if ready {
        for path in session.files(&subject) {
            if let Some(file) = session.load(&path) {
                session.check(file);
            }
        }
    }
    session.conclude(shell, shown, started);
    Ok(session.outcome())
}

mod cache;
mod debug;
mod doc;
mod explain;
#[cfg(not(target_os = "wasi"))]
mod ffi;
mod fix;
mod flash;
mod fmt;
mod inputs;
mod new;
mod pkg;
mod settings;
mod status;
mod target;
mod test;
mod toolchain;
mod util;

#[cfg(test)]
mod tests;
