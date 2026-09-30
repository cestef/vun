//! `vun test [path] [names]...`: builds the program running the tests of a namespace, runs it and
//! reports each test as it ends (spec 11). The path is a file, a directory or a package's
//! directory; with none, the package around the current directory.

use std::io::{self, BufRead, BufReader};
use std::path::Path;
use std::process::Stdio;
use std::time::Instant;

use vun_command::{Args, Arity, Command, Example, Outcome, Param, PathKind, Values};
use vun_data::{Object, ToData};
use vun_driver::Session;
use vun_driver::testing::{Line, Outcome as Ended, Ran, Totals};
use vun_term::{Document, Noun, Record, Shell, Status, Style, Text, Verb};

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

const PASSED: Noun = Noun { one: "passed", many: "passed" };
const TRAPPED: Noun = Noun { one: "trapped", many: "trapped" };

/// A test run and what became of it.
struct Tested(Ran);

impl Record for Tested {
    fn kind(&self) -> &'static str {
        "test"
    }

    fn text(&self) -> Text {
        let name = Text::of(Style::Strong, self.0.name.clone());
        let Ended::Trapped(trapped) = &self.0.outcome else { return name };
        let Some(said) = &trapped.said else { return name };
        let message = said.message.clone().map(|m| Verb::under(Text::of(Style::Error, m)));
        let at = said.at.as_ref().map(|at| {
            Verb::under(Text::of(Style::Scope, format!("at {}:{}:{}", at.file, at.line, at.column)))
        });
        [message, at].into_iter().flatten().fold(name, |text, line| text.newline().append(line))
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("name", self.0.name.clone()).with("outcome", self.0.outcome.to_data())
    }
}

impl Status for Tested {
    fn verb(&self) -> Verb {
        match self.0.outcome {
            Ended::Passed => Verb::good("Passed"),
            Ended::Trapped(_) => Verb::failed("Trapped"),
        }
    }
}

/// How many tests passed and trapped.
struct Tally(Totals);

impl Record for Tally {
    fn kind(&self) -> &'static str {
        "tested"
    }

    fn text(&self) -> Text {
        Text::join(
            [
                Text::from(PASSED.count(self.0.passed).to_string()),
                Text::from(TRAPPED.count(self.0.trapped).to_string()),
            ],
            ", ",
        )
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("passed", self.0.passed).with("trapped", self.0.trapped)
    }
}

impl Status for Tally {
    fn verb(&self) -> Verb {
        if self.0.trapped == 0 { Verb::good("Tested") } else { Verb::failed("Tested") }
    }
}

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
    let mut child = session.command(&program).args(args.values(&NAMES)).stdout(Stdio::piped()).spawn()?;
    let mut totals = None;
    if let Some(out) = child.stdout.take() {
        for text in BufReader::new(out).lines() {
            let text = text?;
            match Line::of(&text) {
                Some(Line::Test(ran)) => shell.status(&Tested(ran)),
                Some(Line::Totals(ended)) => totals = Some(ended),
                // What the tests write themselves.
                None => {
                    shell.result(&Document { kind: "output", name: program.display().to_string(), text })?;
                }
            }
        }
    }
    let status = child.wait()?;
    if let Some(totals) = totals {
        shell.status(&Tally(totals));
    }
    let passed = status.success() && totals.is_some_and(|t| t.trapped == 0);
    Ok(if passed { Outcome::Success } else { Outcome::Failure })
}
