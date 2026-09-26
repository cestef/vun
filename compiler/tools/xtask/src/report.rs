//! The corpus run as `cargo xtask corpus` shows it: a verdict line per program that did not
//! simply pass, the reasons under it, and a tally.

use vun_harness::At;
use vun_term::{Style, Text, Verb};

use crate::corpus::Verdict;

pub(crate) const PASSED: Verb = Verb::good("Passed");
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

    pub(crate) fn text(&self) -> Text {
        let failed = if self.failed == 0 { Style::Good } else { Style::Error };
        let text = Text::of(Style::Good, format!("{} passed", self.passed)).plain(", ");
        let text = text.styled(Style::Warning, format!("{} pending", self.pending));
        let text = text.plain(format!(", {} planned, {} shown, ", self.planned, self.shown));
        let verb = if self.failed == 0 { PASSED } else { FAILED };
        verb.line(text.styled(failed, format!("{} failed", self.failed)))
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
