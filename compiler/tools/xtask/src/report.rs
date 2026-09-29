//! The corpus run as `cargo xtask corpus` shows it: a verdict line per program that did not
//! simply pass, the reasons under it, and a tally.

use vun_data::Object;
use vun_diag::errors::DECLARED_IN;
use vun_harness::At;
use vun_term::{Bytes, Record, Status, Style, Text, Verb};

use crate::corpus::Verdict;

const PASSED: Verb = Verb::good("Passed");
const PENDING: Verb = Verb::warned("Pending");
const PLANNED: Verb = Verb { word: "Planned", style: Style::Note };
const FAILED: Verb = Verb::failed("Failed");
/// The width of the tags of a failure's lines: `unexpected`.
const TAG: usize = 11;

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
            failed => {
                debug_assert!(failed.is_failure());
                &mut self.failed
            }
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
        Object::new()
            .with("path", self.path.clone())
            .with("verdict", self.verdict.name())
            .with("details", self.verdict.details())
    }
}

/// A check of the repository that passed, and what it measured.
pub(crate) enum Passed {
    Layers { groups: usize },
    Lines { max: u32 },
    Size { path: String, bytes: u64, budget: u64 },
}

impl Record for Passed {
    fn kind(&self) -> &'static str {
        match self {
            Self::Layers { .. } => "layers",
            Self::Lines { .. } => "lines",
            Self::Size { .. } => "size",
        }
    }

    fn text(&self) -> Text {
        match self {
            Self::Layers { groups } => {
                Text::from(format!("every crate depends only on its own group or earlier ones, of {groups}"))
            }
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
            Self::Layers { groups } => Object::new().with("groups", *groups),
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

/// Annotations as reports show them: `12:names.duplicate`, or `{ "line": 12, "id": ... }`.
trait Shown {
    fn text(&self) -> Text;
    fn json(&self) -> Object<'static>;
}

impl Shown for At {
    fn text(&self) -> Text {
        Text::of(Style::Accent, format!("{}:", self.line)).styled(Style::Literal, self.id.clone())
    }

    fn json(&self) -> Object<'static> {
        Object::new().with("line", self.line).with("id", self.id.clone())
    }
}

/// Places as JSON, in order.
fn json<'a>(ats: impl IntoIterator<Item = &'a At>) -> Vec<Object<'static>> {
    ats.into_iter().map(At::json).collect()
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

    /// What JSON says of it beside its name: the reasons its lines show, as fields.
    fn details(&self) -> Object<'static> {
        let details = Object::new();
        match self {
            Self::Pass | Self::Shown | Self::Ready => details,
            Self::Pending(later) => details.with("awaits", json(later)),
            Self::Undeclared(ids) => details.with("undeclared", json(ids)),
            Self::Malformed(malformed) => details.with("malformed", malformed.to_string()),
            Self::Unexplained(why) => details.with("unexplained", why.to_string()),
            Self::Planned(waits) => {
                let waits = waits.iter().map(|r| r.at.json().with("text", r.text.clone()));
                details.with("waits", waits.collect::<Vec<_>>())
            }
            Self::Fail(mismatch) => {
                let differing = mismatch.differing.iter().map(|(expected, found)| {
                    let found =
                        found.iter().map(|r| Object::new().with("col", r.col).with("text", r.text.clone()));
                    expected
                        .at
                        .json()
                        .with("col", expected.col)
                        .with("fragments", expected.fragments.clone())
                        .with("found", found.collect::<Vec<_>>())
                });
                details
                    .with("missing", json(&mismatch.missing))
                    .with("unexpected", json(&mismatch.unexpected))
                    .with("differing", differing.collect::<Vec<_>>())
            }
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
                    let col =
                        expected.col.map(|c| Text::from(" at col ").styled(Style::Accent, c.to_string()));
                    let fragments =
                        expected.fragments.iter().map(|f| Text::of(Style::Literal, format!("\"{f}\"")));
                    let wanted = Text::join(fragments, ", ");
                    let head = tagged(
                        "differing",
                        expected
                            .at
                            .text()
                            .append(col.unwrap_or_default())
                            .plain(" with ")
                            .append(wanted)
                            .plain(", found:"),
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
