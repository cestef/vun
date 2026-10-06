//! A benchmark against its baseline: the change of its median, and whether it is more than noise
//! or the threshold `--fail-over` sets (decision 602).

use vun_data::ToData;
use vun_term::Percent;

use super::Statistics;

/// What a change of the median amounts to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ToData)]
pub enum Verdict {
    Improved,
    Unchanged,
    Regressed,
}

/// A benchmark's statistics against those of its baseline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Comparison {
    /// How much the median grew, in percent; negative when it shrank.
    pub change: f64,
    pub verdict: Verdict,
}

impl Comparison {
    /// How `now` compares with `baseline`. A change is more than `fail_over` and more than the
    /// larger spread of the two, so noise alone does not fail a run.
    #[must_use]
    pub fn of(now: &Statistics, baseline: &Statistics, fail_over: Option<Percent>) -> Self {
        let change = if baseline.median > 0.0 {
            (now.median - baseline.median) / baseline.median * 100.0
        } else {
            0.0
        };
        let threshold = fail_over.map_or(0.0, |p| p.0).max(now.spread().0).max(baseline.spread().0);
        let verdict = if change > threshold {
            Verdict::Regressed
        } else if change < -threshold {
            Verdict::Improved
        } else {
            Verdict::Unchanged
        };
        Self { change, verdict }
    }

    /// Whether `--fail-over` asks for the run to fail over it.
    #[must_use]
    pub fn fails(&self, fail_over: Option<Percent>) -> bool {
        fail_over.is_some() && self.verdict == Verdict::Regressed
    }
}
