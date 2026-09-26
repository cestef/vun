//! The corpus run as `cargo xtask corpus` shows it: a verdict line per program that did not
//! simply pass, the reasons under it, and a tally.

use vun_harness::At;
use vun_json::Object;
use vun_term::{Bytes, Record, Status, Style, Text, Verb};

use crate::corpus::Verdict;

const PASSED: Verb = Verb::good("Passed");
const PENDING: Verb = Verb::warned("Pending");
const PLANNED: Verb = Verb { word: "Planned", style: Style::Note };
const FAILED: Verb = Verb::failed("Failed");
/// The width of the tags of a failure's lines: `unexpected`.
const TAG: usize = 11;
/// Where annotations are declared, which an undeclared one points at.
const DECLARED_IN: &str = "lib/lang/lang/errors.vun";

/// How many programs got each verdict.
#[derive(Default)]
pub(crate) struct Tally {
    passed: usize,
    pending: usize,
    planned: usize,
    shown: usize,
    pub(crate) failed: usize,
}

impl Tally {
    pub(crate) fn count(&mut self, verdict: &Verdict) {
        let count = match verdict {
            Verdict::Pass => &mut self.passed,
            Verdict::Pending(_) => &mut self.pending,
            Verdict::Planned(_) => &mut self.planned,
            Verdict::Shown => &mut self.shown,
            Verdict::Fail(_)
            | Verdict::Undeclared(_)
            | Verdict::Malformed(_)
            | Verdict::Unexplained(_)
            | Verdict::Ready => &mut self.failed,
        };
        *count += 1;
    }
}

impl Record for Tally {
    fn kind(&self) -> &'static str {
        "tally"
    }

    fn text(&self) -> Text {
        let failed = if self.failed == 0 { Style::Good } else { Style::Error };
        let text = Text::of(Style::Good, format!("{} passed", self.passed)).plain(", ");
        let text = text.styled(Style::Warning, format!("{} pending", self.pending));
        let text = text.plain(format!(", {} planned, {} shown, ", self.planned, self.shown));
        let verb = if self.failed == 0 { PASSED } else { FAILED };
        verb.line(text.styled(failed, format!("{} failed", self.failed)))
    }

    fn fields(&self) -> Object<'static> {
        let Self { passed, pending, planned, shown, failed } = *self;
        let counts = Object::new().with("passed", passed).with("pending", pending).with("planned", planned);
        counts.with("shown", shown).with("failed", failed)
    }
}

/// A program of the corpus and its verdict.
pub(crate) struct Judged<'v> {
    pub(crate) path: String,
    pub(crate) verdict: &'v Verdict,
}

impl Record for Judged<'_> {
    fn kind(&self) -> &'static str {
        "program"
    }

    fn text(&self) -> Text {
        Text::join(self.verdict.lines(Text::of(Style::Strong, self.path.clone())), "\n")
    }

    /// Its path, its verdict, and the reasons shown under it.
    fn fields(&self) -> Object<'static> {
        let lines = self.verdict.lines(Text::new());
        let details: Vec<String> = lines.iter().skip(1).map(|l| l.to_string().trim().to_owned()).collect();
        Object::new()
            .with("path", self.path.clone())
            .with("verdict", self.verdict.name())
            .with("details", details)
    }
}

/// A check of the repository that passed, and what it measured.
pub(crate) enum Passed {
    Lines { max: u32 },
    Size { path: String, bytes: u64, budget: u64 },
}

impl Record for Passed {
    fn kind(&self) -> &'static str {
        match self {
            Self::Lines { .. } => "lines",
            Self::Size { .. } => "size",
        }
    }

    fn text(&self) -> Text {
        match self {
            Self::Lines { max } => Text::from(format!("every source file has at most {max} lines")),
            Self::Size { path, bytes, budget } => {
                let under = format!(
                    " is {}, {} under the budget of {}",
                    Bytes(*bytes),
                    Bytes(budget - bytes),
                    Bytes(*budget)
                );
                Text::of(Style::Strong, path.clone()).plain(under)
            }
        }
    }

    fn fields(&self) -> Object<'static> {
        match self {
            Self::Lines { max } => Object::new().with("max", *max),
            Self::Size { path, bytes, budget } => {
                Object::new().with("path", path.clone()).with("bytes", *bytes).with("budget", *budget)
            }
        }
    }
}

impl Status for Passed {
    fn verb(&self) -> Verb {
        PASSED
    }
}

/// Annotations as reports show them: `12:names.duplicate`.
trait AtText {
    fn text(&self) -> Text;
}

impl AtText for At {
    fn text(&self) -> Text {
        Text::of(Style::Accent, format!("{}:", self.line)).styled(Style::Literal, self.id.clone())
    }
}

/// A line of a failure: its tag, then what it is about.
fn tagged(tag: &'static str, text: Text) -> Text {
    Verb::under(Text::of(Style::Dim, format!("{tag:<TAG$}")).append(text))
}

impl Verdict {
    /// How JSON names it.
    pub(crate) const fn name(&self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Pending(_) => "pending",
            Self::Fail(_) => "fail",
            Self::Undeclared(_) => "undeclared",
            Self::Malformed(_) => "malformed",
            Self::Shown => "shown",
            Self::Unexplained(_) => "unexplained",
            Self::Planned(_) => "planned",
            Self::Ready => "ready",
        }
    }

    /// The lines reporting this verdict on the program at `path`; none for a pass.
    pub(crate) fn lines(&self, path: Text) -> Vec<Text> {
        let reason = |text: Text| vec![FAILED.line(path.clone()), Verb::under(text)];
        match self {
            Self::Pass | Self::Shown => Vec::new(),
            Self::Pending(later) => vec![
                PENDING.line(path.plain(": awaits ").append(Text::join(later.iter().map(At::text), ", "))),
            ],
            Self::Planned(waits) => {
                let rest = waits.iter().map(|r| {
                    Verb::under(
                        Text::of(Style::Accent, format!("{}: ", r.at.line))
                            .plain(r.text.replace('\n', " | ")),
                    )
                });
                std::iter::once(PLANNED.line(path.plain(": waits for"))).chain(rest).collect()
            }
            Self::Ready => {
                reason(Text::from("checks clean; move it to ").styled(Style::Strong, vun_harness::ACCEPT))
            }
            Self::Unexplained(why) => reason(Text::from(why.to_string())),
            Self::Malformed(malformed) => reason(Text::from(format!("malformed annotation: {malformed}"))),
            Self::Undeclared(ids) => {
                let declare = Text::of(Style::Dim, format!(" (declare it in {DECLARED_IN})"));
                let lines = ids.iter().map(|a| tagged("undeclared", a.text().append(declare.clone())));
                std::iter::once(FAILED.line(path)).chain(lines).collect()
            }
            Self::Fail(mismatch) => {
                let missing = mismatch.missing.iter().map(|a| tagged("missing", a.text()));
                let unexpected = mismatch.unexpected.iter().map(|a| tagged("unexpected", a.text()));
                let differing = mismatch.differing.iter().flat_map(|(expected, found)| {
                    let head = tagged(
                        "differing",
                        expected.at.text().plain(format!(" expected {expected:?}, found:")),
                    );
                    let cols = found.iter().map(|r| {
                        Verb::under(
                            Text::of(Style::Accent, format!("  col {}: ", r.col))
                                .plain(r.text.replace('\n', " | ")),
                        )
                    });
                    std::iter::once(head).chain(cols)
                });
                std::iter::once(FAILED.line(path)).chain(missing).chain(unexpected).chain(differing).collect()
            }
        }
    }
}
