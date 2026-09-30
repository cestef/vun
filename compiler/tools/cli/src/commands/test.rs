//! `vun test [path] [names]...`: builds the program running the tests of a namespace, runs it and
//! reports each test as it ends (spec 11). The path is a file, a directory or a package's
//! directory; with none, the package around the current directory.

use std::io;
use std::path::Path;
use std::time::Instant;

use vun_command::{Args, Arity, Command, Example, Outcome, Param, PathKind, Values};
use vun_driver::Session;
use vun_term::Shell;

use super::status::{Conclude, Started, Step};
use super::{COMPILE, Inputs};

const PATH: Param = Param::positional("path")
    .arity(Arity::Optional)
    .about("a file, a directory or a package's directory; the package around this directory if none")
    .values(Values::Path(PathKind::Any));
const NAMES: Param =
    Param::positional("names").arity(Arity::Many).about("the tests to run, by name; every one if none");

pub(super) const TEST: Command<Session> = Command::run("test", "run the tests of a namespace", test)
    .aliases(&["t"])
    .params(&[
        PATH,
        NAMES,
        vun_driver::codegen::BACKEND,
        vun_driver::codegen::LINKER,
        vun_driver::program::OUTPUT,
    ])
    .sets(&[COMPILE])
    .examples(&[
        Example { line: "", about: "run the tests of the package around this directory" },
        Example { line: "net/", about: "run the tests of a directory and the namespaces in it" },
        Example { line: "main.vun crc_vector", about: "run one test of a file" },
    ]);

fn test(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let started = Instant::now();
    let path = args.value(&PATH).map(Path::new);
    let Some(root) = session.tests_root(path)? else { return Ok(session.outcome()) };
    let subject = path.unwrap_or(&root).display().to_string();
    shell.status(&Started { step: Step::Build, subject: subject.clone() });
    let output = args.value(&vun_driver::program::OUTPUT).map(Path::new);
    let program = session.root_at(&root, shell).and_then(|file| {
        let ns = session.tested(file, path)?;
        session.build_tests(file, ns, output).ok()
    });
    session.conclude(shell, subject, started);
    let Some(program) = program else { return Ok(session.outcome()) };
    shell.status(&Started { step: Step::Run, subject: program.display().to_string() });
    let reported = session.run_tests(&program, args.values(&NAMES))?.report(shell)?;
    Ok(if reported.passed { Outcome::Success } else { Outcome::Failure })
}
