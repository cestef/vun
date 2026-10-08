//! `vun debug compare`: a program built and run through every backend that generates code: what
//! each one cost (code generation, the program's size, its run time), how it ended and what it
//! wrote, and for each that parts from the reference, the first line where it does.

use std::io;
use std::path::Path;
use std::time::Duration;

use vun_command::{Args, Outcome, Param, Values};
use vun_data::Object;
use vun_driver::Session;
use vun_driver::compare::{Difference, Ending, Ran};
use vun_term::{Bytes, Column, Elapsed, Nanos, Record, Row, Shell, Status, Style, Text, Verb};

use super::FILE;

const RUNS: Param = Param::option("runs")
    .about("how many times each program runs; the run time shown is the median (1 by default)")
    .values(Values::Count);

pub(super) const PARAMS: &[Param] = &[FILE, RUNS];

/// How a backend's program ended, as people read it.
fn exit(ending: Ending) -> Text {
    match ending {
        Ending::Returned(0) => Text::of(Style::Good, "0"),
        Ending::Returned(code) => Text::of(Style::Warning, code.to_string()),
        Ending::Trapped => Text::of(Style::Warning, "trap"),
        Ending::Crashed(signal) => Text::of(Style::Error, format!("signal {signal}")),
    }
}

fn nanos(time: Duration) -> u64 {
    u64::try_from(time.as_nanos()).unwrap_or(u64::MAX)
}

/// One backend's program, set against the reference's, the run most backends agree with.
struct Backend<'a> {
    ran: &'a Ran,
    difference: Option<Difference>,
    reference: bool,
}

impl Record for Backend<'_> {
    fn kind(&self) -> &'static str {
        "backend"
    }

    fn text(&self) -> Text {
        Text::join(self.cells(), "  ")
    }

    fn fields(&self) -> Object<'static> {
        let Ran { backend, output, size, built, runs } = self.ran;
        Object::new()
            .with("backend", *backend)
            .with("exit", output.status.code())
            .with("ending", format!("{:?}", self.ran.ending()))
            .with("stdout", output.stdout.len())
            .with("stderr", output.stderr.len())
            .with("size", *size)
            .with("built_nanos", nanos(*built))
            .with("run_nanos", nanos(self.ran.median()))
            .with("runs", runs.len())
            .with("reference", self.reference)
            .with("agrees", self.difference.is_none())
    }
}

impl Row for Backend<'_> {
    const COLUMNS: &'static [Column] = &[
        Column::left("backend"),
        Column::right("exit"),
        Column::right("stdout"),
        Column::right("stderr"),
        Column::right("code"),
        Column::right("built"),
        Column::right("run"),
        Column::left("vs reference"),
    ];

    fn cells(&self) -> Vec<Text> {
        let versus = match (&self.difference, self.reference) {
            (_, true) => Text::of(Style::Dim, "reference"),
            (None, false) => Text::of(Style::Good, "same"),
            (Some(_), false) => Text::of(Style::Error, "differs"),
        };
        vec![
            Text::of(Style::Literal, self.ran.backend),
            exit(self.ran.ending()),
            Text::from(Bytes(self.ran.output.stdout.len() as u64).to_string()),
            Text::from(Bytes(self.ran.output.stderr.len() as u64).to_string()),
            Text::from(Bytes(self.ran.size).to_string()),
            Text::from(Elapsed(self.ran.built).to_string()),
            Text::from(Nanos::from(nanos(self.ran.median())).to_string()),
            versus,
        ]
    }
}

/// Where one backend parts from the reference.
struct Parting<'a> {
    reference: &'a str,
    other: &'a str,
    difference: Difference,
}

impl Parting<'_> {
    /// A line of `backend`, or that its stream had ended.
    fn line(mark: &'static str, style: Style, backend: &str, line: Option<&str>) -> Text {
        let shown = match line {
            Some(line) => Text::from(line.to_owned()),
            None => Text::of(Style::Dim, "(the stream ended)"),
        };
        Text::of(style, mark).plain(format!(" {backend}: ")).append(shown)
    }
}

impl Record for Parting<'_> {
    fn kind(&self) -> &'static str {
        "parting"
    }

    fn text(&self) -> Text {
        let head = Text::of(Style::Literal, self.other.to_owned())
            .plain(" parts from ")
            .styled(Style::Literal, self.reference.to_owned());
        match &self.difference {
            Difference::Ending { reference, other } => {
                head.plain(": ended ").append(exit(*other)).plain(" where it was ").append(exit(*reference))
            }
            Difference::Terminator { stream, reference } => head.plain(format!(
                ": the last line of {} {} a line break",
                stream.name(),
                if *reference { "lacks" } else { "ends in" }
            )),
            Difference::Output { stream, line, reference, other } => head
                .plain(format!(" at line {line} of {}", stream.name()))
                .newline()
                .append(Self::line("-", Style::Error, self.reference, reference.as_deref()))
                .newline()
                .append(Self::line("+", Style::Good, self.other, other.as_deref())),
        }
    }

    fn fields(&self) -> Object<'static> {
        let base =
            Object::new().with("reference", self.reference.to_owned()).with("backend", self.other.to_owned());
        match &self.difference {
            Difference::Ending { reference, other } => base
                .with("what", "ending")
                .with("expected", format!("{reference:?}"))
                .with("found", format!("{other:?}")),
            Difference::Terminator { stream, reference } => {
                base.with("what", stream.name()).with("reference_terminated", *reference)
            }
            Difference::Output { stream, line, reference, other } => base
                .with("what", stream.name())
                .with("line", *line)
                .with("expected", reference.clone())
                .with("found", other.clone()),
        }
    }
}

/// What the comparison came to.
struct Verdict {
    backends: usize,
    parting: usize,
}

impl Record for Verdict {
    fn kind(&self) -> &'static str {
        "verdict"
    }

    fn text(&self) -> Text {
        if self.parting == 0 {
            Text::of(Style::Strong, format!("{} backends agree", self.backends))
        } else {
            Text::of(
                Style::Strong,
                format!("{} of {} backends differ from the reference", self.parting, self.backends),
            )
        }
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("backends", self.backends).with("differing", self.parting)
    }
}

impl Status for Verdict {
    fn verb(&self) -> Verb {
        if self.parting == 0 { Verb::good("Agreed") } else { Verb::failed("Differ") }
    }
}

pub(super) fn compare(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let path = args.value(&FILE).expect("positional parameters are present when a command runs");
    let times = args.count(&RUNS).and_then(|n| usize::try_from(n).ok()).unwrap_or(1);
    let Some(file) = session.load(Path::new(path)) else { return Ok(session.outcome()) };
    let Ok(runs) = session.run_each(file, times) else { return Ok(session.outcome()) };
    if runs.is_empty() {
        return Ok(Outcome::Success);
    }
    let held = Ran::consensus(&runs).0;
    let reference = &runs[held];
    let rows: Vec<Backend<'_>> = runs
        .iter()
        .enumerate()
        .map(|(i, ran)| Backend { ran, difference: ran.differs(reference), reference: i == held })
        .collect();
    shell.table(&rows)?;
    let partings: Vec<Parting<'_>> = rows
        .iter()
        .filter_map(|b| {
            let difference = b.difference.clone()?;
            Some(Parting { reference: reference.backend, other: b.ran.backend, difference })
        })
        .collect();
    for parting in &partings {
        shell.result(parting)?;
    }
    let verdict = Verdict { backends: runs.len(), parting: partings.len() };
    shell.status(&verdict);
    Ok(if partings.is_empty() { Outcome::Success } else { Outcome::Failure })
}
