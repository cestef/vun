//! Running the tests' program and reporting what it writes: each test as it ends, with the lines
//! it wrote while it ran, then the totals.

use std::ffi::OsStr;
use std::io::{self, BufRead, BufReader, PipeReader};
use std::path::{Path, PathBuf};
use std::process::Child;

use vun_data::{Object, ToData};
use vun_term::{Document, Noun, Record, Shell, Status, Style, Text, Verb};

use super::{Line, Outcome, Ran, Totals, Trapped};
use crate::Session;

const PASSED: Noun = Noun { one: "passed", many: "passed" };
const TRAPPED: Noun = Noun { one: "trapped", many: "trapped" };
const IGNORED: Noun = Noun { one: "ignored", many: "ignored" };

/// The tests' program running, its standard output and error read as one stream: what a test
/// writes, the backtrace of its trap included, comes before its line.
pub struct Run {
    program: PathBuf,
    child: Child,
    lines: BufReader<PipeReader>,
}

impl Session {
    /// Starts `program`, the tests' program, running the tests `names`, every one if none.
    ///
    /// # Errors
    /// Fails when the program cannot be started.
    pub fn run_tests<S: AsRef<OsStr>>(
        &self,
        program: &Path,
        names: impl IntoIterator<Item = S>,
    ) -> io::Result<Run> {
        let (reader, writer) = io::pipe()?;
        let child = self.command(program).args(names).stdout(writer.try_clone()?).stderr(writer).spawn()?;
        Ok(Run { program: program.to_path_buf(), child, lines: BufReader::new(reader) })
    }
}

/// What a run of the tests' program reported: the tests run, and whether it ran to its totals
/// and no test trapped.
pub struct Reported {
    pub ran: Vec<Ran>,
    pub passed: bool,
}

impl Run {
    /// Reports each test as it ends, with what it wrote, then the totals; what the program wrote
    /// after its last test is reported as its output.
    ///
    /// # Errors
    /// Fails when the program's output cannot be read or reported.
    pub fn report(mut self, shell: &mut Shell<'_>) -> io::Result<Reported> {
        let (mut totals, mut output, mut tests) = (None, Vec::new(), Vec::new());
        for text in self.lines.lines() {
            let text = text?;
            match Line::of(&text) {
                Some(Line::Test(ran)) => {
                    let tested = Tested { ran, output: std::mem::take(&mut output) };
                    shell.status(&tested);
                    tests.push(tested.ran);
                }
                Some(Line::Totals(ended)) => totals = Some(ended),
                None => output.push(text),
            }
        }
        if !output.is_empty() {
            let name = self.program.display().to_string();
            shell.result(&Document { kind: "output", name, text: output.join("\n") })?;
        }
        let status = self.child.wait()?;
        if let Some(totals) = totals {
            shell.status(&Tally(totals));
        }
        Ok(Reported { ran: tests, passed: status.success() && totals.is_some_and(|t| t.trapped == 0) })
    }
}

/// A test run, what became of it and the lines the program wrote while it ran.
struct Tested {
    ran: Ran,
    output: Vec<String>,
}

impl Tested {
    /// What its outcome says: why it was not run, or the trap's message and place.
    fn said(&self) -> Vec<Text> {
        match &self.ran.outcome {
            Outcome::Passed | Outcome::Trapped(Trapped { said: None, .. }) => Vec::new(),
            Outcome::Ignored(ignored) => vec![Text::of(Style::Warning, ignored.reason.clone())],
            Outcome::Trapped(Trapped { said: Some(said), .. }) => {
                let message = said.message.clone().map(|m| Text::of(Style::Error, m));
                let at = said
                    .at
                    .as_ref()
                    .map(|at| Text::of(Style::Scope, format!("at {}:{}:{}", at.file, at.line, at.column)));
                [message, at].into_iter().flatten().collect()
            }
        }
    }
}

impl Record for Tested {
    fn kind(&self) -> &'static str {
        "test"
    }

    fn text(&self) -> Text {
        let output = self.output.iter().map(|line| Text::from(line.clone()));
        let lines = self.said().into_iter().chain(output);
        lines.fold(Text::of(Style::Strong, self.ran.name.clone()), |text, line| {
            text.newline().append(Verb::under(line))
        })
    }

    fn fields(&self) -> Object<'static> {
        Object::new()
            .with("name", self.ran.name.clone())
            .with("outcome", self.ran.outcome.to_data())
            .with("output", self.output.clone())
    }
}

impl Status for Tested {
    fn verb(&self) -> Verb {
        match self.ran.outcome {
            Outcome::Passed => Verb::good("Passed"),
            Outcome::Ignored(_) => Verb::warned("Ignored"),
            Outcome::Trapped(_) => Verb::failed("Trapped"),
        }
    }
}

/// How many tests passed, trapped and were not run.
struct Tally(Totals);

impl Record for Tally {
    fn kind(&self) -> &'static str {
        "tested"
    }

    fn text(&self) -> Text {
        let Totals { passed, trapped, ignored } = self.0;
        let counts = [PASSED.count(passed), TRAPPED.count(trapped), IGNORED.count(ignored)];
        Text::join(counts.map(|count| Text::from(count.to_string())), ", ")
    }

    fn fields(&self) -> Object<'static> {
        let Totals { passed, trapped, ignored } = self.0;
        Object::new().with("passed", passed).with("trapped", trapped).with("ignored", ignored)
    }
}

impl Status for Tally {
    fn verb(&self) -> Verb {
        if self.0.trapped == 0 { Verb::good("Tested") } else { Verb::failed("Tested") }
    }
}
