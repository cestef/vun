//! A bar: one task's share of the work, advanced from any thread.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use super::Progress;
use crate::{Noun, Text, Verb};

/// What a bar counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    /// Things of one kind, such as files: `12/40 files`.
    Items(Noun),
    /// Bytes, such as a download: `3.4 MiB/12.0 MiB`.
    Bytes,
}

pub(crate) struct Task {
    pub(super) verb: Verb,
    pub(super) subject: Mutex<Text>,
    pub(super) total: AtomicU64,
    pub(super) done: AtomicU64,
    pub(super) unit: Unit,
    pub(super) started: Instant,
    /// What the task says it works on, asked for each time it is drawn, instead of `subject`.
    pub(super) live: Option<Live>,
    /// How long the task runs before it is drawn: work that ends sooner shows nothing.
    pub(super) delay: Duration,
}

/// Says what a task works on now; nothing when it does not.
pub type Live = Box<dyn Fn() -> Text + Send + Sync>;

impl Task {
    pub(super) fn new(verb: Verb, subject: Text, total: u64, unit: Unit) -> Self {
        Self {
            verb,
            subject: Mutex::new(subject),
            total: AtomicU64::new(total),
            done: AtomicU64::new(0),
            unit,
            started: Instant::now(),
            live: None,
            delay: Duration::ZERO,
        }
    }
}

/// A task's progress. Dropping it ends the task.
pub struct Bar {
    progress: Progress,
    task: Arc<Task>,
}

impl Bar {
    pub(super) const fn new(progress: Progress, task: Arc<Task>) -> Self {
        Self { progress, task }
    }

    /// Counts `by` more units done.
    pub fn advance(&self, by: u64) {
        self.task.done.fetch_add(by, Ordering::Relaxed);
    }

    /// Counts the units done so far.
    #[must_use]
    pub fn done(&self) -> u64 {
        self.task.done.load(Ordering::Relaxed)
    }

    /// Says how many units there are, once that is known.
    pub fn total(&self, total: u64) {
        self.task.total.store(total, Ordering::Relaxed);
    }

    /// Names what the task is working on now.
    pub fn subject(&self, subject: Text) {
        *self.task.subject.lock().unwrap_or_else(PoisonError::into_inner) = subject;
    }
}

impl Drop for Bar {
    fn drop(&mut self) {
        self.progress.remove(&self.task);
    }
}
