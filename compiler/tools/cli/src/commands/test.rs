//! `vun test [path] [names]... [--seed n] [--cases n]`: builds the program running the tests of a
//! namespace, runs it and reports each test as it ends (spec 11). The path is a file, a directory
//! or a package's directory; with none, the package around the current directory, which a first
//! word naming nothing on disk is the first test's name for. `vun bench`
//! runs the benchmarks alone, measured, through the same program, and `vun fuzz` the properties
//! alone, fuzzed, through it built covered.

use std::io;
use std::path::Path;
use std::time::Instant;

use vun_command::{Args, Arity, Command, Example, Outcome, Param};
use vun_driver::Session;
use vun_driver::subject::Subject;
use vun_driver::testing::Mode;
use vun_sema::cover::Coverage;
use vun_term::Shell;

use super::status::{Conclude, Started, Step};
use super::{COMPILE, Inputs, PATH};

const NAMES: Param =
    Param::positional("names").arity(Arity::Many).about("the tests to run, by name; every one if none");

pub(super) const TEST: Command<Session> = Command::run("test", "run the tests of a namespace", test)
    .aliases(&["t"])
    .params(&[
        PATH,
        NAMES,
        vun_driver::testing::SEED,
        vun_driver::testing::CASES,
        vun_driver::codegen::BACKEND,
        vun_driver::codegen::LINKER,
        vun_driver::program::OUTPUT,
    ])
    .sets(&[COMPILE])
    .examples(&[
        Example { line: "", about: "run the tests of the package around this directory" },
        Example { line: "net/", about: "run the tests of a directory and the namespaces in it" },
        Example { line: "main.vun crc_vector", about: "run one test of a file" },
        Example { line: "main.vun roundtrip --seed 42", about: "replay a property's cases from a seed" },
    ]);

pub(super) const BENCH: Command<Session> =
    Command::run("bench", "run the benchmarks of a namespace, measured", bench)
        .params(&[
            PATH,
            NAMES,
            vun_driver::codegen::BACKEND,
            vun_driver::codegen::LINKER,
            vun_driver::program::OUTPUT,
        ])
        .sets(&[COMPILE])
        .examples(&[
            Example { line: "", about: "measure the benchmarks of the package around this directory" },
            Example { line: "main.vun memcpy_4k", about: "measure one benchmark of a file" },
        ]);

pub(super) const FUZZ: Command<Session> =
    Command::run("fuzz", "fuzz the properties of a namespace, guided by what their cases cover", fuzz)
        .params(&[
            PATH,
            NAMES,
            vun_driver::testing::SEED,
            vun_driver::testing::FUZZED,
            vun_driver::codegen::BACKEND,
            vun_driver::codegen::LINKER,
            vun_driver::program::OUTPUT,
        ])
        .sets(&[COMPILE])
        .examples(&[
            Example { line: "", about: "fuzz the properties of the package around this directory" },
            Example { line: "main.vun decode_raw --cases 1000000", about: "fuzz one property longer" },
        ]);

fn test(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    tests(session, args, shell, Mode::Test)
}

fn bench(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    tests(session, args, shell, Mode::Bench)
}

fn fuzz(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    session.cover(Coverage::Blocks);
    tests(session, args, shell, Mode::Fuzz)
}

/// What the path and names of a command line say: a first word naming nothing on disk is the
/// first test's name, the path being the package around this directory.
pub(super) struct Chosen<'a> {
    pub(super) path: Option<&'a Path>,
    pub(super) names: Vec<&'a str>,
}

impl<'a> Chosen<'a> {
    pub(super) fn of(args: &'a Args) -> Self {
        let path = args.value(&PATH);
        let (path, first) = match path {
            Some(word) if !Path::new(word).exists() => (None, Some(word)),
            _ => (path.map(Path::new), None),
        };
        Self { path, names: first.into_iter().chain(args.values(&NAMES)).collect() }
    }
}

/// Builds the tests' program of each package the path names and runs it in `mode`: the tests
/// the arguments name, the benchmarks alone, measured, or the properties alone, fuzzed.
fn tests(session: &mut Session, args: &Args, shell: &mut Shell<'_>, mode: Mode) -> io::Result<Outcome> {
    let chosen = Chosen::of(args);
    let Some(subject) = session.subject(chosen.path)? else { return Ok(session.outcome()) };
    let mut passed = true;
    for (root, within) in subject.tested() {
        let place = (root.as_path(), within.as_deref());
        let Some(reported) = tested(session, args, &chosen.names, shell, mode, (&subject, place))? else {
            return Ok(session.outcome());
        };
        passed &= reported;
    }
    Ok(if passed { Outcome::Success } else { Outcome::Failure })
}

/// Builds and runs the tests of the namespace `within` names in the package of `root`, its root
/// namespace when `None`, once the files `covered` holds are checked; whether they passed, `None`
/// when they did not build.
fn tested(
    session: &mut Session,
    args: &Args,
    names: &[&str],
    shell: &mut Shell<'_>,
    mode: Mode,
    (covered, (root, within)): (&Subject, (&Path, Option<&Path>)),
) -> io::Result<Option<bool>> {
    let started = Instant::now();
    let shown = Subject::relative(within.unwrap_or(root));
    let announced = Started { step: Step::Build, subject: shown.clone() };
    let output = args.value(&vun_driver::program::OUTPUT).map(Path::new);
    // The files of what the path names are checked as `vun check` does, not only those the
    // tests reach.
    let program = session
        .root_at(root, Some(&announced), shell)
        .filter(|_| session.checked(covered))
        .and_then(|file| {
            let ns = session.tested(file, within)?;
            session.build_tests(file, ns, output).ok()
        });
    session.conclude(shell, shown, started);
    let Some(program) = program else { return Ok(None) };
    shell.status(&Started { step: Step::Run, subject: Subject::relative(&program) });
    let options = vun_driver::testing::Options::of(args, mode);
    let reported = session.run_tests(&program, names.iter().copied(), options)?.report(shell)?;
    Ok(Some(reported.passed))
}
