//! What `vun bench` reports: each benchmark's statistics and how they compare with the baseline,
//! and what was left out or kept (spec 11, section 5).

use std::path::PathBuf;

use vun_data::{Object, ToData};
use vun_term::{Noun, Record, Status, Style, Text, Verb};

use super::counter::{Reason, Unavailable};
use super::{Comparison, Measure, Statistics, Verdict};

const SAMPLES: Noun = Noun { one: "sample", many: "samples" };
const OUTLIERS: Noun = Noun { one: "outlier", many: "outliers" };

/// A benchmark measured: its statistics, and how they compare with its baseline's.
pub(super) struct Benchmark {
    pub name: String,
    pub measure: Measure,
    pub statistics: Statistics,
    pub against: Option<(String, Comparison)>,
}

impl Record for Benchmark {
    fn kind(&self) -> &'static str {
        "benchmark"
    }

    fn text(&self) -> Text {
        let Statistics { samples, outliers, median, .. } = self.statistics;
        let shown = self.measure.show(median);
        let name = Text::of(Style::Strong, self.name.clone());
        let mut said = vec![format!(
            "{shown} per iteration, spread {}, over {}",
            self.statistics.spread(),
            SAMPLES.count(samples)
        )];
        if outliers > 0 {
            said.push(format!("{} left out of the mean", OUTLIERS.count(outliers)));
        }
        if let Some((baseline, comparison)) = &self.against {
            said.push(format!("{:+.1}% against `{baseline}`", comparison.change));
        }
        said.into_iter().fold(name, |text, line| text.newline().append(Verb::under(Text::from(line))))
    }

    fn fields(&self) -> Object<'static> {
        let object = Object::new().with("name", self.name.clone()).with("measure", self.measure.to_data());
        let object = object.with("statistics", self.statistics.to_data());
        match &self.against {
            Some((baseline, Comparison { change, verdict })) => object
                .with("baseline", baseline.clone())
                .with("change", change.to_data())
                .with("verdict", verdict.to_data()),
            None => object,
        }
    }
}

impl Status for Benchmark {
    fn verb(&self) -> Verb {
        match self.against.as_ref().map(|(_, c)| c.verdict) {
            Some(Verdict::Regressed) => Verb::failed("Regressed"),
            Some(Verdict::Improved) => Verb::good("Improved"),
            Some(Verdict::Unchanged) | None => Verb::good("Measured"),
        }
    }
}

/// `--measure instructions` is time here, and why.
pub(super) struct Fallback(pub Unavailable);

impl Record for Fallback {
    fn kind(&self) -> &'static str {
        "bench_fallback"
    }

    fn text(&self) -> Text {
        Text::from(format!("{}, so benchmarks are timed", self.0))
    }

    fn fields(&self) -> Object<'static> {
        let reason: Reason = self.0.reason();
        Object::new().with("measure", Measure::Instructions.to_data()).with("reason", reason.to_data())
    }
}

impl Status for Fallback {
    fn verb(&self) -> Verb {
        Verb::warned("Counting")
    }
}

/// A baseline that holds another measure than this run's is not compared with.
pub(super) struct Mismatched {
    pub name: String,
    pub measure: Measure,
}

impl Record for Mismatched {
    fn kind(&self) -> &'static str {
        "bench_mismatch"
    }

    fn text(&self) -> Text {
        Text::from(format!("baseline `{}` holds {}, not compared with", self.name, self.measure.name()))
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("baseline", self.name.clone()).with("measure", self.measure.to_data())
    }
}

impl Status for Mismatched {
    fn verb(&self) -> Verb {
        Verb::warned("Ignored")
    }
}

/// A baseline written.
pub(super) struct Saved {
    pub name: String,
    pub path: PathBuf,
    pub benchmarks: usize,
}

impl Record for Saved {
    fn kind(&self) -> &'static str {
        "bench_saved"
    }

    fn text(&self) -> Text {
        Text::from(format!("baseline `{}` at {}", self.name, self.path.display()))
    }

    fn fields(&self) -> Object<'static> {
        let path = self.path.display().to_string();
        Object::new()
            .with("baseline", self.name.clone())
            .with("path", path)
            .with("benchmarks", self.benchmarks)
    }
}

impl Status for Saved {
    fn verb(&self) -> Verb {
        Verb::good("Saved")
    }
}
