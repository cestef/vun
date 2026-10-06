//! Timing for another process's table: a component's library has its own, which nobody reads, so
//! the host lends it a relay, and what the library times, on any of its threads, is timed in the
//! host's table.

use std::sync::OnceLock;

use super::Activity;

/// Where the activities of a process are timed instead.
pub trait Relay: Send + Sync {
    fn begin(&self, activity: Activity);
    /// The innermost activity the calling thread began ends.
    fn end(&self);
}

static RELAY: OnceLock<Box<dyn Relay>> = OnceLock::new();

/// The relay of the process, if it has one.
pub struct Relaying;

impl Relaying {
    /// Times every activity of this process through `relay` from now on: the first relay
    /// installed is the one.
    pub fn install(relay: Box<dyn Relay>) {
        let _ = RELAY.set(relay);
    }

    /// Hands `activity` to the relay: whether there is one.
    pub(super) fn begin(activity: Activity) -> bool {
        RELAY.get().inspect(|relay| relay.begin(activity)).is_some()
    }

    pub(super) fn end() {
        if let Some(relay) = RELAY.get() {
            relay.end();
        }
    }
}
