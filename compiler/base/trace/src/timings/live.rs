//! The stages running now, for a display that says what a command is doing. Off until a display
//! asks, when starting a stage costs one relaxed load.

use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::atomic::{AtomicBool, Ordering};

use super::Activity;

static ON: AtomicBool = AtomicBool::new(false);
/// The stages running, in the order they started, and how many threads run each.
static RUNNING: Mutex<Vec<(&'static str, usize)>> = Mutex::new(Vec::new());

/// The stages running now.
pub struct Live;

impl Live {
    /// Starts keeping track.
    pub fn watch() {
        ON.store(true, Ordering::Relaxed);
    }

    /// The names of the stages running, the longest running first.
    #[must_use]
    pub fn stages() -> Vec<&'static str> {
        RUNNING.lock().unwrap_or_else(PoisonError::into_inner).iter().map(|(name, _)| *name).collect()
    }

    /// Notes that a thread started `activity`, if it is a stage and anyone watches.
    pub(super) fn enter(activity: Activity) -> Option<&'static str> {
        if !ON.load(Ordering::Relaxed) || activity.kind != super::Kind::Stage {
            return None;
        }
        let mut running = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);
        match running.iter_mut().find(|(name, _)| *name == activity.name) {
            Some((_, threads)) => *threads += 1,
            None => running.push((activity.name, 1)),
        }
        Some(activity.name)
    }

    /// Notes that a thread ended the stage `name`.
    pub(super) fn leave(name: &'static str) {
        let mut running = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(at) = running.iter().position(|(n, _)| *n == name) {
            running[at].1 -= 1;
            if running[at].1 == 0 {
                running.remove(at);
            }
        }
    }
}
