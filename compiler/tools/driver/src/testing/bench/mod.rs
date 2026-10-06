//! `vun bench` (spec 11, section 5): the benchmarks of a program measured over several samples,
//! the statistics of each, and a comparison with a baseline kept in `.vun/bench/`.

mod baseline;
mod compare;
mod counter;
mod measure;
mod report;
mod sampling;
mod statistics;

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use vun_command::{Args, Param, Values};
use vun_term::{Percent, Shell};

pub use baseline::Baseline;
pub use compare::{Comparison, Verdict};
pub use measure::Measure;
pub use statistics::Statistics;

use super::{Mode, Options};
use crate::Session;
use report::{Benchmark, Fallback, Mismatched, Saved};
use sampling::Sampled;

pub const MEASURE: Param = Param::option("measure")
    .about("what to measure: wall-clock `time`, or the `instructions` a process retires, where it can be counted")
    .values(Values::Choice(Measure::names));
pub const SAMPLES: Param = Param::option("samples")
    .about("how many runs of the benchmarks to take samples from; 10 if not given")
    .values(Values::Count);
pub const BASELINE: Param = Param::option("baseline")
    .about("the baseline to compare with, and to save as; `base` if not given")
    .values(Values::Text);
pub const SAVE: Param = Param::flag("save").about("keep these results as the baseline, in `.vun/bench/`");
pub const FAIL_OVER: Param = Param::option("fail-over")
    .about("fail when a benchmark's median grew by more than this, and by more than the noise, over the baseline")
    .values(Values::Percent);

/// How many samples a benchmark has unless `--samples` asks for another count.
const SAMPLED: usize = 10;
/// The baseline `--baseline` names unless it names another.
const DEFAULT_BASELINE: &str = "base";

/// What `vun bench` was asked for besides the benchmarks it names.
pub struct Plan {
    pub measure: Measure,
    pub samples: usize,
    pub baseline: String,
    pub save: bool,
    pub fail_over: Option<Percent>,
}

impl Plan {
    /// The plan a command line gives.
    #[must_use]
    pub fn of(args: &Args) -> Self {
        let measure = args.value(&MEASURE).and_then(Measure::named).unwrap_or_default();
        let samples =
            args.count(&SAMPLES).and_then(|n| usize::try_from(n).ok()).map_or(SAMPLED, |n| n.max(1));
        Self {
            measure,
            samples,
            baseline: args.value(&BASELINE).unwrap_or(DEFAULT_BASELINE).to_owned(),
            save: args.flag(&SAVE),
            fail_over: args.percent(&FAIL_OVER),
        }
    }
}

impl Session {
    /// Measures the benchmarks `names` of `program`, every one if none, as `plan` says, reports
    /// each with its comparison with the baseline of the package in the directory `root`, and keeps the
    /// results as the baseline when asked; whether every benchmark ran and none regressed past
    /// `--fail-over`.
    ///
    /// # Errors
    /// Fails when the program cannot be run or the baseline read or written.
    pub fn bench(
        &self,
        program: &Path,
        root: &Path,
        names: &[&str],
        plan: &Plan,
        shell: &mut Shell<'_>,
    ) -> io::Result<bool> {
        let options = Options { mode: Mode::Bench, ..Options::default() };
        let (measure, sampling) = match plan.measure {
            Measure::Time => (Measure::Time, self.sample_time(program, names, plan.samples, options, shell)?),
            Measure::Instructions => {
                match self.sample_instructions(program, names, plan.samples, options, shell)? {
                    Ok(sampling) => (Measure::Instructions, sampling),
                    Err(unavailable) => {
                        shell.status(&Fallback(unavailable));
                        (Measure::Time, self.sample_time(program, names, plan.samples, options, shell)?)
                    }
                }
            }
        };
        let mut passed = sampling.passed;
        let baseline = Baseline::read(root, &plan.baseline).map_err(io::Error::other)?;
        let baseline = baseline.filter(|kept| {
            let same = kept.measure == measure;
            if !same {
                shell.status(&Mismatched { name: plan.baseline.clone(), measure: kept.measure });
            }
            same
        });
        let mut kept = Baseline { measure, benchmarks: BTreeMap::new() };
        for Sampled { name, values } in sampling.benchmarks {
            let Some(statistics) = Statistics::of(&values) else { continue };
            let against = baseline.as_ref().and_then(|b| {
                let comparison = Comparison::of(&statistics, b.benchmarks.get(&name)?, plan.fail_over);
                passed &= !comparison.fails(plan.fail_over);
                Some((plan.baseline.clone(), comparison))
            });
            shell.status(&Benchmark { name: name.clone(), measure, statistics, against });
            kept.benchmarks.insert(name, statistics);
        }
        if plan.save && sampling.passed {
            let benchmarks = kept.benchmarks.len();
            let path = kept.write(root, &plan.baseline).map_err(io::Error::other)?;
            shell.status(&Saved { name: plan.baseline.clone(), path, benchmarks });
        }
        Ok(passed)
    }
}

#[cfg(test)]
mod tests;
