//! `vun test [path] [names]... [--seed n] [--cases n]`: builds the program running the tests of a
//! namespace, runs it and reports each test as it ends (spec 11). The path is a file, a directory
//! or a package's directory; with none, the package around the current directory, which a first
//! word naming nothing on disk is the first test's name for. `vun bench`
//! runs the benchmarks alone, measured over samples, through the same program, and `vun fuzz` the properties
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

pub(super) const NAMES: Param =
    Param::positional("names").arity(Arity::Many).about("the tests to run, by name; every one if none");

pub(super) const TEST: Command<Session> = Command::run("test", "run the tests of a namespace", test)
    .aliases(&["t"])
    .params(&[
        PATH,
        NAMES,
        vun_driver::testing::SEED,
        vun_driver::testing::CASES,
        vun_driver::testing::JOBS,
        vun_driver::codegen::BACKEND,
        vun_driver::codegen::LINKER,
        vun_driver::program::OUTPUT,
    ])
    .sets(&[COMPILE])
    .examples(&[
        Example { line: "", about: "run the tests of the package around this directory" },
        Example { line: "--jobs 1", about: "run them in one process, in the order they are declared" },
        Example { line: "net/", about: "run the tests of a directory and the namespaces in it" },
        Example { line: "main.vun crc_vector", about: "run one test of a file" },
        Example { line: "main.vun roundtrip --seed 42", about: "replay a property's cases from a seed" },
    ]);

pub(super) const BENCH: Command<Session> =
    Command::run("bench", "run the benchmarks of a namespace, measured", bench)
        .params(&[
            PATH,
            NAMES,
            vun_driver::testing::MEASURE,
            vun_driver::testing::SAMPLES,
            vun_driver::testing::BASELINE,
            vun_driver::testing::SAVE,
            vun_driver::testing::FAIL_OVER,
            vun_driver::codegen::BACKEND,
            vun_driver::codegen::LINKER,
            vun_driver::program::OUTPUT,
        ])
        .sets(&[COMPILE])
        .examples(&[
            Example { line: "", about: "measure the benchmarks of the package around this directory" },
            Example { line: "main.vun memcpy_4k", about: "measure one benchmark of a file" },
            Example { line: "--save", about: "keep the results as the baseline, in `.vun/bench/`" },
            Example {
                line: "--fail-over 3%",
                about: "fail when a benchmark got more than 3% slower than the baseline",
            },
            Example {
                line: "--measure instructions",
                about: "count instructions, which noise does not move",
            },
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
pub(super) fn tests(
    session: &mut Session,
    args: &Args,
    shell: &mut Shell<'_>,
    mode: Mode,
) -> io::Result<Outcome> {
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
            let program = session.build_tests(file, ns.clone(), output).ok()?;
            Some((program, file, ns))
        });
    session.conclude(shell, shown, started);
    let Some((program, file, ns)) = program else { return Ok(None) };
    shell.status(&Started { step: Step::Run, subject: Subject::relative(&program) });
    if mode == Mode::Bench {
        let plan = vun_driver::testing::Plan::of(args);
        let package = root.parent().unwrap_or(root);
        return Ok(Some(session.bench(&program, package, names, &plan, shell)?));
    }
    let options = vun_driver::testing::Options::of(args, mode);
    // Tests run each in a process of its own, as many at once as asked; benchmarks and fuzzing
    // measure and guide, so they run alone and in order.
    let jobs = args
        .count(&vun_driver::testing::JOBS)
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, usize::from));
    if mode == Mode::Test && jobs > 1 {
        let listed = session.list_tests(&program, names.iter().copied(), options)?;
        if listed.len() > 1 {
            let reported = session.run_tests_parallel(&program, &listed, options, jobs, shell)?;
            return Ok(Some(reported.passed));
        }
    }
    let reported = session.run_tests(&program, names.iter().copied(), options)?.report(shell)?;
    if mode == Mode::Cover {
        super::coverage::report(
            session,
            args,
            shell,
            (file, &ns),
            (&reported, covered, Subject::scope(root)),
        )?;
    }
    Ok(Some(reported.passed))
}
