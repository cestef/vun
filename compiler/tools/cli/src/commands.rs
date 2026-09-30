//! The `vun` command tree (spec 12, section 7), shared by every front end of the driver.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;

use vun_command::{Args, Arity, Command, Example, OptionSet, Outcome, Param, PathKind, Program, Values};
use vun_span::FileId;
use vun_term::Shell;

use status::{Conclude, Report, Started, Step};

use vun_build::layout::MANIFEST;
use vun_driver::subject::Subject;
use vun_driver::{Session, VERSION, checks, codegen, formats, trace};

pub(crate) const PROGRAM: Program<Session> = Program {
    version: VERSION,
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
            CHECK,
            fix::FIX,
            doc::DOC,
            explain::EXPLAIN,
            fmt::FMT,
            test::TEST,
            test::BENCH,
            Command::group(pkg::GROUP, "dependencies and releases", pkg::COMMANDS),
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
    trace::configure(session, args);
    vun_driver::target::configure(session, args)
}

/// What a command's path names, once its project is ready, and how it is shown.
struct Named {
    subject: Subject,
    shown: String,
    /// Whether the project is ready: its dependencies fetched and its lockfile written.
    ready: bool,
}

trait Inputs {
    /// What the path `args` gives names, with none the package around this directory, for `step`:
    /// the command handed to the toolchain the project pins if it pins another, `step` announced,
    /// then the project made ready; `None` once why nothing is named is reported.
    fn named(&mut self, args: &Args, step: Step, shell: &mut Shell<'_>) -> io::Result<Option<Named>>;

    /// Makes the project around the file at `path` ready, as cargo does before compiling:
    /// dependencies fetched and the lockfile written, after handing the command to the
    /// toolchain the project pins if it pins another. `started` is shown once this toolchain is
    /// the one to run, so a handed-off command announces it once.
    fn ready(&mut self, path: &Path, started: Option<&Started>, shell: &mut Shell<'_>) -> bool;

    /// Reads the file at `path` once its project is ready.
    fn root_at(&mut self, path: &Path, started: Option<&Started>, shell: &mut Shell<'_>) -> Option<FileId>;

    /// Builds what the path `args` gives names, reporting how it went: the program of a file, or
    /// the programs the plan of each package records.
    fn built(&mut self, args: &Args, shell: &mut Shell<'_>) -> io::Result<Option<Vec<PathBuf>>>;
}

impl Inputs for Session {
    fn named(&mut self, args: &Args, step: Step, shell: &mut Shell<'_>) -> io::Result<Option<Named>> {
        let path = args.value(&PATH).map(Path::new);
        let Some(subject) = self.subject(path)? else { return Ok(None) };
        let shown = path.map_or_else(|| subject.shown(), |p| p.display().to_string());
        let ready = self.ready(&subject.root(), Some(&Started { step, subject: shown.clone() }), shell);
        Ok(Some(Named { subject, shown, ready }))
    }

    fn ready(&mut self, path: &Path, started: Option<&Started>, shell: &mut Shell<'_>) -> bool {
        // A project pinning another toolchain is that one's to build.
        if self.hand_off(path).is_err() {
            return false;
        }
        if let Some(started) = started {
            shell.status(started);
        }
        for (name, hash, fetched) in self.prepare(path) {
            fetched.report(name, hash, shell);
        }
        !self.failed()
    }

    fn root_at(&mut self, path: &Path, started: Option<&Started>, shell: &mut Shell<'_>) -> Option<FileId> {
        self.ready(path, started, shell).then(|| self.load(path)).flatten()
    }

    fn built(&mut self, args: &Args, shell: &mut Shell<'_>) -> io::Result<Option<Vec<PathBuf>>> {
        let started = Instant::now();
        let Some(Named { subject, shown, ready }) = self.named(args, Step::Build, shell)? else {
            return Ok(None);
        };
        let output = args.value(&vun_driver::program::OUTPUT).map(Path::new);
        let built = if !ready {
            None
        } else if let Some(file) = subject.file() {
            self.load(file).and_then(|file| self.build(file, output).ok()).map(|p| vec![p])
        } else {
            // A package's plan records its programs; a workspace builds each package's.
            let plans = subject.packages().into_iter().map(|dir| {
                let manifest = self.load(&dir.join(MANIFEST))?;
                self.carry_out(manifest).ok()
            });
            plans.collect::<Option<Vec<_>>>().map(|built| built.concat())
        };
        let concluded = match built.as_deref() {
            Some([program]) => Subject::relative(program),
            _ => shown,
        };
        self.conclude(shell, concluded, started);
        Ok(built)
    }
}

fn build(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    session.built(args, shell)?;
    Ok(session.outcome())
}

fn run(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let Some(built) = session.built(args, shell)? else { return Ok(session.outcome()) };
    let Some(program) = session.to_run(built) else { return Ok(session.outcome()) };
    shell.status(&Started { step: Step::Run, subject: Subject::relative(&program) });
    let status = session.command(&program).args(args.values(&ARGS)).status()?;
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
mod ffi;
mod fix;
mod fmt;
mod pkg;
mod status;
mod target;
mod test;
mod toolchain;
mod util;

#[cfg(test)]
mod tests;
