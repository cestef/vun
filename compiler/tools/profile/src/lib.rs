//! Where a compile spends its instructions and its time, measured so that two runs compare.

pub mod callgrind;
mod entry;
mod error;
mod report;
mod run;
mod sample;
mod snapshot;
mod study;
mod workload;

pub use entry::{Entries, Entry, Key, Scope};
pub use error::MeasureError;
pub use run::Runner;
pub use sample::{Sample, Timed};
pub use snapshot::{Snapshot, SnapshotError};
pub use study::{Studied, Study, StudyError};
pub use workload::{Phase, Setup, Verb, Weight, Workload};

#[cfg(test)]
mod tests;
