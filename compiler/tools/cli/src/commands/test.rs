//! `vun test [path] [names]... [--seed n] [--cases n]`: builds the program running the tests of a
//! namespace, runs it and reports each test as it ends (spec 11). The path is a file, a directory
//! or a package's directory; with none, the package around the current directory. `vun bench`
//! runs the benchmarks alone, measured, through the same program.

use std::io;
use std::path::Path;
use std::time::Instant;

use vun_command::{Args, Arity, Command, Example, Outcome, Param};
use vun_driver::Session;
use vun_driver::subject::Subject;
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

fn test(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    tests(session, args, shell, false)
}

fn bench(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    tests(session, args, shell, true)
}

/// Builds the tests' program of each package the path names and runs it: the tests the arguments
/// name, or the benchmarks alone, measured, when `bench`.
fn tests(session: &mut Session, args: &Args, shell: &mut Shell<'_>, bench: bool) -> io::Result<Outcome> {
    let path = args.value(&PATH).map(Path::new);
    let Some(subject) = session.subject(path)? else { return Ok(session.outcome()) };
    let mut passed = true;
    for (root, within) in subject.tested() {
        let Some(reported) = tested(session, args, shell, bench, &root, within.as_deref())? else {
            return Ok(session.outcome());
        };
        passed &= reported;
    }
    Ok(if passed { Outcome::Success } else { Outcome::Failure })
}

/// Builds and runs the tests of the namespace `within` names in the package of `root`, its root
/// namespace when `None`; whether they passed, `None` when they did not build.
fn tested(
    session: &mut Session,
    args: &Args,
    shell: &mut Shell<'_>,
    bench: bool,
    root: &Path,
    within: Option<&Path>,
) -> io::Result<Option<bool>> {
    let started = Instant::now();
    let subject = Subject::relative(within.unwrap_or(root));
    let announced = Started { step: Step::Build, subject: subject.clone() };
    let output = args.value(&vun_driver::program::OUTPUT).map(Path::new);
    let program = session.root_at(root, Some(&announced), shell).and_then(|file| {
        let ns = session.tested(file, within)?;
        session.build_tests(file, ns, output).ok()
    });
    session.conclude(shell, subject, started);
    let Some(program) = program else { return Ok(None) };
    shell.status(&Started { step: Step::Run, subject: Subject::relative(&program) });
    let options = vun_driver::testing::Options::of(args, bench);
    let reported = session.run_tests(&program, args.values(&NAMES), options)?.report(shell)?;
    Ok(Some(reported.passed))
}
