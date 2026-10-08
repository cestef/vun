//! A run's timings: the compiler asked for `--timings` as JSON, read back as typed rows.

use std::time::Duration;

use vun_data::{FromData, json};
use vun_trace::Kind;

use crate::error::MeasureError;

/// The record the compiler writes its timings in.
const RECORD: &str = "timings";
/// The member naming a record's kind.
const TYPE: &str = "type";

/// What the compiler wrote of one activity.
#[derive(FromData)]
struct Written {
    kind: String,
    name: String,
    runs: u64,
    own_us: u64,
    total_us: u64,
}

#[derive(FromData)]
struct Report {
    wall_us: u64,
    work_us: u64,
    activities: Vec<Written>,
}

/// An activity's time in one run, summed over the threads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Timed {
    pub kind: Kind,
    pub name: String,
    pub runs: u64,
    pub own: Duration,
    pub total: Duration,
}

/// One run of the compiler.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sample {
    pub wall: Duration,
    /// Own times summed: the work done, over every thread.
    pub work: Duration,
    pub activities: Vec<Timed>,
}

impl Sample {
    /// A run known only by its duration, such as a program's.
    #[must_use]
    pub fn whole(wall: Duration) -> Self {
        Self { wall, work: wall, activities: Vec::new() }
    }

    /// The sample in the output of a run with `--timings --format json`.
    ///
    /// # Errors
    /// Fails when the output holds no timings, or they do not fit.
    pub fn read(output: &str) -> Result<Self, MeasureError> {
        let report = output
            .lines()
            .filter_map(|line| json::parse(line).ok())
            .find(|value| value[TYPE].as_str() == Some(RECORD))
            .ok_or(MeasureError::Unreported)?;
        let Report { wall_us, work_us, activities } =
            Report::from_data(&report).map_err(MeasureError::Shape)?;
        let micros = Duration::from_micros;
        let activities = activities
            .into_iter()
            .filter_map(|w| {
                let kind = Kind::ALL.iter().copied().find(|k| k.to_string() == w.kind)?;
                Some(Timed {
                    kind,
                    name: w.name,
                    runs: w.runs,
                    own: micros(w.own_us),
                    total: micros(w.total_us),
                })
            })
            .collect();
        Ok(Self { wall: micros(wall_us), work: micros(work_us), activities })
    }
}
