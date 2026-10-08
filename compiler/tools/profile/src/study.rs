//! Measuring a workload and reporting it: the run's total, then what it was spent on, set against
//! a snapshot when there is one.

use std::io;

use vun_driver::testing::{Comparison, Measure, Statistics};
use vun_term::{Percent, Shell};

use crate::callgrind::{Function, Profile};
use crate::entry::{Entries, Entry, Scope};
use crate::error::MeasureError;
use crate::report::{Against, Callees, Calling, Extra, Ranked, Total};
use crate::run::Runner;
use crate::sample::Sample;
use crate::snapshot::Snapshot;
use crate::workload::{Phase, Workload};

/// How many callers are listed for a function.
const CALLERS: usize = 3;
/// The sources listed: the rest are a long tail of the standard library and the system.
const SOURCES: usize = 16;

/// What is measured, how, and what it is compared with.
pub struct Study<'a> {
    pub runner: Runner<'a>,
    pub measure: Measure,
    pub samples: usize,
    /// How many functions and activities are listed.
    pub top: usize,
    /// How many functions a snapshot keeps.
    pub keep: usize,
    pub callers: bool,
    pub baseline: Option<Snapshot>,
    /// The change that is more than noise, unless `fail_over` is more.
    pub tolerance: Percent,
    pub fail_over: Option<Percent>,
}

/// What a study of one workload and phase came to.
pub struct Studied {
    /// What a snapshot keeps.
    pub entries: Vec<Entry>,
    pub regressed: bool,
}

impl Study<'_> {
    /// Measures `workload` in `phase` and reports it.
    ///
    /// # Errors
    /// Fails when the measurement does or the output does.
    pub fn run(
        &self,
        workload: &Workload,
        phase: Phase,
        shell: &mut Shell<'_>,
    ) -> Result<Studied, StudyError> {
        match self.measure {
            Measure::Instructions => {
                let profile = self.runner.count(workload, phase)?;
                Ok(self.instructions(&Entries { workload, phase }, &profile, shell)?)
            }
            Measure::Time => {
                let samples = self.runner.sample(workload, phase, self.samples)?;
                Ok(self.times(&Entries { workload, phase }, &samples, shell)?)
            }
        }
    }

    fn against(&self, entry: &Entry) -> Option<Against> {
        let kept = self.baseline.as_ref()?.find(&entry.key)?;
        let threshold = self.fail_over.unwrap_or(self.tolerance);
        Some(Against { comparison: Comparison::of(&entry.statistics, &kept.statistics, Some(threshold)) })
    }

    fn regressed(&self, against: Option<Against>) -> bool {
        against.is_some_and(|a| a.comparison.fails(self.fail_over))
    }

    fn rank(&self, entry: Entry, whole: f64, extra: Extra) -> Ranked {
        let Statistics { median, .. } = entry.statistics;
        let against = self.against(&entry);
        Ranked {
            share: Percent(if whole > 0.0 { median / whole * 100.0 } else { 0.0 }),
            entry,
            extra,
            against,
        }
    }

    fn total(&self, entries: &[Entry], shell: &mut Shell<'_>) -> (f64, bool) {
        let mut regressed = false;
        let mut whole = 0.0;
        for entry in entries.iter().filter(|e| matches!(e.key.scope, Scope::Total | Scope::Work)) {
            let against = self.against(entry);
            regressed |= self.regressed(against);
            if entry.key.scope == Scope::Work || self.measure == Measure::Instructions {
                whole = entry.statistics.median;
            }
            shell.status(&Total { entry: entry.clone(), against });
        }
        (whole, regressed)
    }

    fn instructions(
        &self,
        entries: &Entries<'_>,
        profile: &Profile,
        shell: &mut Shell<'_>,
    ) -> io::Result<Studied> {
        let kept = entries.of_profile(profile, self.keep);
        let (whole, regressed) = self.total(&kept, shell);
        let sources: Vec<Ranked> = kept
            .iter()
            .filter(|e| e.key.scope == Scope::Source)
            .take(SOURCES)
            .map(|e| self.rank(e.clone(), whole, Extra::default()))
            .collect();
        shell.table(&sources)?;
        let functions: Vec<(usize, Ranked)> = profile
            .functions
            .iter()
            .enumerate()
            .take(self.top)
            .filter_map(|(at, f)| {
                let entry =
                    kept.iter().find(|e| e.key.scope == Scope::Function && e.key.name == f.name.as_str())?;
                #[expect(clippy::cast_precision_loss, reason = "counts stay far below 2^52")]
                let extra = Extra {
                    callees: f.inclusive.map_or(Callees::Cycle, |n| Callees::Ran(n as f64)),
                    count: Some(f.calls),
                };
                Some((at, self.rank(entry.clone(), whole, extra)))
            })
            .collect();
        let regressed = regressed || functions.iter().any(|(_, r)| self.regressed(r.against));
        let (positions, rows): (Vec<usize>, Vec<Ranked>) = functions.into_iter().unzip();
        shell.table(&rows)?;
        if self.callers {
            let calling: Vec<Calling> = positions
                .into_iter()
                .flat_map(|at| Self::calling(profile, at, entries.workload.name))
                .collect();
            shell.table(&calling)?;
        }
        Ok(Studied { entries: kept, regressed })
    }

    fn calling(profile: &Profile, callee: usize, workload: &str) -> Vec<Calling> {
        let Function { name, inclusive, .. } = profile.functions[callee];
        let inclusive = inclusive.unwrap_or(0);
        profile
            .callers(callee)
            .into_iter()
            .take(CALLERS)
            .map(|edge| Calling {
                workload: workload.to_owned(),
                caller: profile.functions[edge.caller].name.as_str().to_owned(),
                callee: name.as_str().to_owned(),
                calls: edge.calls,
                cost: edge.cost,
                #[expect(clippy::cast_precision_loss, reason = "a share")]
                share: edge.cost.map(|cost| {
                    Percent(if inclusive > 0 { cost as f64 / inclusive as f64 * 100.0 } else { 0.0 })
                }),
            })
            .collect()
    }

    fn times(&self, entries: &Entries<'_>, samples: &[Sample], shell: &mut Shell<'_>) -> io::Result<Studied> {
        let kept = entries.of_samples(samples);
        let (whole, regressed) = self.total(&kept, shell);
        let runs = |name: &str| -> Option<u64> {
            let first = samples.first()?;
            first.activities.iter().find(|a| format!("{} {}", a.kind, a.name) == name).map(|a| a.runs)
        };
        let mut ranked: Vec<&Entry> = kept.iter().filter(|e| e.key.scope == Scope::Activity).collect();
        ranked.sort_by(|a, b| b.statistics.median.total_cmp(&a.statistics.median));
        let rows: Vec<Ranked> = ranked
            .into_iter()
            .take(self.top)
            .map(|e| {
                self.rank(e.clone(), whole, Extra { callees: Callees::Unknown, count: runs(&e.key.name) })
            })
            .collect();
        let regressed = regressed || rows.iter().any(|r| self.regressed(r.against));
        shell.table(&rows)?;
        Ok(Studied { entries: kept, regressed })
    }
}

/// Why a study did not come to a report.
#[derive(Debug)]
pub enum StudyError {
    Measure(MeasureError),
    Output(io::Error),
}

impl std::fmt::Display for StudyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Measure(_) => write!(f, "the measurement failed"),
            Self::Output(_) => write!(f, "the report is not written"),
        }
    }
}

impl std::error::Error for StudyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Measure(source) => Some(source),
            Self::Output(source) => Some(source),
        }
    }
}

impl From<MeasureError> for StudyError {
    fn from(error: MeasureError) -> Self {
        Self::Measure(error)
    }
}

impl From<io::Error> for StudyError {
    fn from(error: io::Error) -> Self {
        Self::Output(error)
    }
}
