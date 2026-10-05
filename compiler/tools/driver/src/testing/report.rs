//! Running the tests' program and reporting what it writes: each test as it ends, with the lines
//! it wrote while it ran, then the totals.

use std::ffi::OsStr;
use std::io::{self, BufRead, BufReader, PipeReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ExitStatus};

use vun_data::{Object, ToData};
use vun_term::{Document, Nanos, Noun, Record, Shell, Status, Style, Text, Verb};

use super::{Argument, Falsified, Fuzzed, Line, Options, Outcome, Ran, Totals};
use crate::Session;

const PASSED: Noun = Noun { one: "passed", many: "passed" };
const TRAPPED: Noun = Noun { one: "trapped", many: "trapped" };
const FALSIFIED: Noun = Noun { one: "falsified", many: "falsified" };
const IGNORED: Noun = Noun { one: "ignored", many: "ignored" };
const MEASURED: Noun = Noun { one: "measured", many: "measured" };
const ITERATIONS: Noun = Noun { one: "iteration", many: "iterations" };
const FUZZED: Noun = Noun { one: "fuzzed", many: "fuzzed" };
const CASES: Noun = Noun { one: "case", many: "cases" };
const STREAMS: Noun = Noun { one: "stream", many: "streams" };
const FLAGS: Noun = Noun { one: "flag", many: "flags" };

/// The tests' program running, its standard output and error read as one stream: what a test
/// writes, the backtrace of its trap included, comes before its line.
pub struct Run {
    program: PathBuf,
    child: Child,
    lines: BufReader<PipeReader>,
}

impl Session {
    /// Starts `program`, the tests' program, running the tests `names`, every one if none, as
    /// `options` ask.
    ///
    /// # Errors
    /// Fails when the program cannot be started.
    pub fn run_tests<S: AsRef<OsStr>>(
        &self,
        program: &Path,
        names: impl IntoIterator<Item = S>,
        options: Options,
    ) -> io::Result<Run> {
        self.start_tests(
            program,
            names.into_iter().map(|n| n.as_ref().to_os_string()).collect(),
            options,
            false,
        )
    }

    /// Starts `program` running the tests `names` and `options` ask for, or only listing them.
    pub(super) fn start_tests(
        &self,
        program: &Path,
        names: Vec<std::ffi::OsString>,
        options: Options,
        list: bool,
    ) -> io::Result<Run> {
        let (reader, writer) = io::pipe()?;
        let mut command = self.command(program);
        command.args(names).args(options.args());
        if list {
            command.arg(super::LIST.long_form());
        }
        let child = command.stdout(writer.try_clone()?).stderr(writer).spawn()?;
        Ok(Run { program: program.to_path_buf(), child, lines: BufReader::new(reader) })
    }
}

/// What a run of the tests' program reported: the tests run, and whether it ran to its totals
/// and no test trapped or was falsified.
pub struct Reported {
    pub tests: Vec<Tested>,
    pub passed: bool,
}

/// What a run of the tests' program wrote, read: the tests, the totals if it got to them, the lines
/// no test claimed, and how the program ended.
pub struct Collected {
    pub program: PathBuf,
    pub tests: Vec<Tested>,
    pub totals: Option<Totals>,
    pub output: Vec<String>,
    pub status: ExitStatus,
}

impl Collected {
    /// Whether the program ran to its totals and no test trapped or was falsified.
    #[must_use]
    pub fn clean(&self) -> bool {
        self.status.success() && self.totals.is_some_and(|t| t.trapped == 0 && t.falsified == 0)
    }
}

impl Run {
    /// The lines the program writes, until it ends; the program is left to be waited for.
    pub(super) fn lines(self) -> io::Result<Vec<io::Result<String>>> {
        let Self { mut child, lines, .. } = self;
        let all = lines.lines().collect();
        child.wait()?;
        Ok(all)
    }

    /// Reads what the program writes until it ends, calling `ended` with each test as it ends.
    ///
    /// # Errors
    /// Fails when the program's output cannot be read.
    pub fn collect(mut self, ended: &mut dyn FnMut(&Tested)) -> io::Result<Collected> {
        let (mut totals, mut output, mut arguments, mut tests) = (None, Vec::new(), Vec::new(), Vec::new());
        for text in self.lines.lines() {
            let text = text?;
            match Line::of(&text) {
                Some(Line::Test(ran)) => {
                    let (arguments, output) = (std::mem::take(&mut arguments), std::mem::take(&mut output));
                    let tested = Tested { ran, arguments, output };
                    ended(&tested);
                    tests.push(tested);
                }
                Some(Line::Argument(argument)) => arguments.push(argument),
                Some(Line::Totals(done)) => totals = Some(done),
                Some(Line::Listed(_)) => {}
                None => output.push(text),
            }
        }
        let status = self.child.wait()?;
        Ok(Collected { program: self.program, tests, totals, output, status })
    }

    /// Reports each test as it ends, with what it wrote, then the totals; what the program wrote
    /// after its last test is reported as its output.
    ///
    /// # Errors
    /// Fails when the program's output cannot be read or reported.
    pub fn report(self, shell: &mut Shell<'_>) -> io::Result<Reported> {
        let collected = self.collect(&mut |tested| shell.status(tested))?;
        if !collected.output.is_empty() {
            let name = collected.program.display().to_string();
            shell.result(&Document { kind: "output", name, text: collected.output.join("\n") })?;
        }
        if let Some(totals) = collected.totals {
            shell.status(&Tally(totals));
        }
        let passed = collected.clean();
        Ok(Reported { tests: collected.tests, passed })
    }
}

/// A test run, what became of it, the arguments of a falsified property's failing case and the
/// lines the program wrote while it ran.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tested {
    pub ran: Ran,
    pub arguments: Vec<Argument>,
    pub output: Vec<String>,
}

impl Tested {
    /// What its outcome says: why it was not run, or the trap's message and place, for a
    /// property the case that failed, for a benchmark what it measured, and for a property fuzzed
    /// what it covered.
    fn said(&self) -> Vec<Text> {
        let outcome = &self.ran.outcome;
        let ignored = match outcome {
            Outcome::Ignored(ignored) => Some(Text::of(Style::Warning, ignored.reason.clone())),
            _ => None,
        };
        let said = outcome.trap().and_then(|t| t.said.as_ref());
        let message = said.and_then(|s| s.message.clone()).map(|m| Text::of(Style::Error, m));
        let at = said
            .and_then(|s| s.at.as_ref())
            .map(|at| Text::of(Style::Scope, format!("at {}:{}:{}", at.file, at.line, at.column)));
        let case = match outcome {
            Outcome::Falsified(Falsified { seed, case, shrinks, .. }) => {
                let below = if self.arguments.is_empty() { "" } else { ", with the arguments below" };
                Some(Text::of(
                    Style::Scope,
                    format!("case {case} of seed {seed:#018x}, shrunk {shrinks} times{below}"),
                ))
            }
            _ => None,
        };
        let measured = match outcome {
            Outcome::Measured(measured) => {
                let iterations = ITERATIONS.count(usize::try_from(measured.iterations).unwrap_or(usize::MAX));
                Some(Text::from(format!(
                    "{} per iteration, over {iterations}",
                    Nanos(measured.per_iteration())
                )))
            }
            _ => None,
        };
        let fuzzed = match outcome {
            Outcome::Fuzzed(Fuzzed { runs, kept, flags }) => Some(Text::from(format!(
                "{}, {} kept, setting {} of coverage",
                CASES.count(usize::try_from(*runs).unwrap_or(usize::MAX)),
                STREAMS.count(*kept),
                FLAGS.count(*flags)
            ))),
            _ => None,
        };
        let arguments = self.arguments.iter().map(|a| Text::from(a.to_string()));
        let said = [ignored, message, at, case, measured, fuzzed];
        said.into_iter().flatten().chain(arguments).collect()
    }
}

impl Record for Tested {
    fn kind(&self) -> &'static str {
        "test"
    }

    fn text(&self) -> Text {
        let output = self.output.iter().map(|line| Text::from(line.clone()));
        let lines = self.said().into_iter().chain(output);
        let name = Text::of(Style::Strong, self.ran.name.clone());
        let name = match self.ran.nanos {
            Some(nanos) => name.append(Text::of(Style::Dim, format!(" {}", Nanos::from(nanos)))),
            None => name,
        };
        lines.fold(name, |text, line| text.newline().append(Verb::under(line)))
    }

    fn fields(&self) -> Object<'static> {
        Object::new()
            .with("name", self.ran.name.clone())
            .with("outcome", self.ran.outcome.to_data())
            .with("nanos", self.ran.nanos.to_data())
            .with("arguments", self.arguments.to_data())
            .with("output", self.output.clone())
    }
}

impl Status for Tested {
    fn verb(&self) -> Verb {
        match self.ran.outcome {
            Outcome::Passed => Verb::good("Passed"),
            Outcome::Ignored(_) => Verb::warned("Ignored"),
            Outcome::Trapped(_) => Verb::failed("Trapped"),
            Outcome::Falsified(_) => Verb::failed("Falsified"),
            Outcome::Measured(_) => Verb::good("Measured"),
            Outcome::Fuzzed(_) => Verb::good("Fuzzed"),
        }
    }
}

/// How many tests passed, trapped, were falsified, were not run, were measured and were fuzzed.
pub(super) struct Tally(pub(super) Totals);

impl Record for Tally {
    fn kind(&self) -> &'static str {
        "tested"
    }

    fn text(&self) -> Text {
        let Totals { passed, trapped, falsified, ignored, measured, fuzzed, nanos } = self.0;
        let counts = [
            PASSED.count(passed),
            TRAPPED.count(trapped),
            FALSIFIED.count(falsified),
            IGNORED.count(ignored),
            MEASURED.count(measured),
            FUZZED.count(fuzzed),
        ];
        let counts = Text::join(counts.map(|count| Text::from(count.to_string())), ", ");
        match nanos {
            Some(nanos) => counts.append(Text::of(Style::Dim, format!(" in {}", Nanos::from(nanos)))),
            None => counts,
        }
    }

    fn fields(&self) -> Object<'static> {
        let Totals { passed, trapped, falsified, ignored, measured, fuzzed, nanos } = self.0;
        let object = Object::new().with("passed", passed).with("trapped", trapped);
        let object = object.with("falsified", falsified).with("ignored", ignored);
        object.with("measured", measured).with("fuzzed", fuzzed).with("nanos", nanos.to_data())
    }
}

impl Status for Tally {
    fn verb(&self) -> Verb {
        if self.0.trapped == 0 && self.0.falsified == 0 {
            Verb::good("Tested")
        } else {
            Verb::failed("Tested")
        }
    }
}
