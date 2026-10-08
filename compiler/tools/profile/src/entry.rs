//! What a measurement keeps: a quantity for each subject of a run, in the form a later run is
//! compared with.

use vun_data::{FromData, ToData};
use vun_driver::testing::{Measure, Statistics};
use vun_variants::Variants;

use crate::callgrind::Profile;
use crate::sample::Sample;
use crate::workload::{Phase, Workload};

/// What a quantity is of.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Variants, ToData, FromData)]
pub enum Scope {
    /// The whole run: its wall time, or all its instructions.
    Total,
    /// The own times of the activities summed over the threads.
    Work,
    /// A crate, or an object, the instructions ran in.
    Source,
    /// A function of the compiler.
    Function,
    /// A stage or query of the compiler.
    Activity,
}

/// Which quantity of which run.
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct Key {
    pub workload: String,
    pub phase: Phase,
    pub measure: Measure,
    pub scope: Scope,
    /// The subject within the scope, as it is shown; empty for the run.
    pub name: String,
}

/// A quantity, measured in nanoseconds or instructions.
#[derive(Clone, Debug, PartialEq, ToData, FromData)]
pub struct Entry {
    pub key: Key,
    pub statistics: Statistics,
}

/// The entries of runs of one workload and phase.
pub struct Entries<'a> {
    pub workload: &'a Workload,
    pub phase: Phase,
}

impl Entries<'_> {
    fn entry(&self, measure: Measure, scope: Scope, name: String, values: &[f64]) -> Option<Entry> {
        let key = Key { workload: self.workload.name.to_owned(), phase: self.phase, measure, scope, name };
        Some(Entry { key, statistics: Statistics::of(values)? })
    }

    /// The instructions of a profile: the run, each source, and the `top` functions.
    #[must_use]
    pub fn of_profile(&self, profile: &Profile, top: usize) -> Vec<Entry> {
        let measure = Measure::Instructions;
        #[expect(clippy::cast_precision_loss, reason = "counts stay far below 2^52")]
        let count = |n: u64| [n as f64];
        let run = self.entry(measure, Scope::Total, String::new(), &count(profile.total));
        let sources = profile
            .sources
            .iter()
            .filter_map(|s| self.entry(measure, Scope::Source, s.source.to_string(), &count(s.instructions)));
        let functions =
            profile.functions.iter().take(top).filter_map(|f| {
                self.entry(measure, Scope::Function, f.name.as_str().to_owned(), &count(f.own))
            });
        run.into_iter().chain(sources).chain(functions).collect()
    }

    /// The times of samples: the run's wall time and work, and each activity's own time, in
    /// nanoseconds per sample.
    #[must_use]
    pub fn of_samples(&self, samples: &[Sample]) -> Vec<Entry> {
        let measure = Measure::Time;
        let nanos = |f: &dyn Fn(&Sample) -> std::time::Duration| -> Vec<f64> {
            samples.iter().map(|s| f(s).as_secs_f64() * 1e9).collect()
        };
        let mut names: Vec<(vun_trace::Kind, &str)> =
            samples.iter().flat_map(|s| &s.activities).map(|a| (a.kind, a.name.as_str())).collect();
        names.sort();
        names.dedup();
        let activities = names.into_iter().filter_map(|(kind, name)| {
            let own: Vec<f64> = samples
                .iter()
                .map(|s| {
                    let found = s.activities.iter().find(|a| a.kind == kind && a.name == name);
                    found.map_or(0.0, |a| a.own.as_secs_f64() * 1e9)
                })
                .collect();
            self.entry(measure, Scope::Activity, format!("{kind} {name}"), &own)
        });
        let wall = self.entry(measure, Scope::Total, String::new(), &nanos(&|s| s.wall));
        let work = self.entry(measure, Scope::Work, String::new(), &nanos(&|s| s.work));
        wall.into_iter().chain(work).chain(activities).collect()
    }
}
