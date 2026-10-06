//! Timing for another process's table: a component's library has its own, which nobody reads, so
//! the host lends the thread of a call a relay, and what the library times is timed in the host's
//! table, nested under the call that is running.

use std::cell::RefCell;

use super::Activity;

/// Where the activities of this thread are timed instead.
pub trait Relay {
    fn begin(&self, activity: Activity);
    fn end(&self);
}

thread_local! {
    static RELAY: RefCell<Option<Box<dyn Relay>>> = const { RefCell::new(None) };
}

/// A relay installed on this thread, removed when dropped.
#[must_use = "the relay is installed until it is dropped"]
pub struct Relaying(());

impl Relaying {
    /// Times this thread's activities through `relay`, until the guard returned is dropped.
    pub fn to(relay: Box<dyn Relay>) -> Self {
        RELAY.with_borrow_mut(|slot| *slot = Some(relay));
        Self(())
    }

    /// Hands `activity` to the relay: whether there is one.
    pub(super) fn begin(activity: Activity) -> bool {
        RELAY.with_borrow(|slot| slot.as_deref().inspect(|relay| relay.begin(activity)).is_some())
    }

    pub(super) fn end() {
        RELAY.with_borrow(|slot| slot.iter().for_each(|relay| relay.end()));
    }
}

impl Drop for Relaying {
    fn drop(&mut self) {
        RELAY.with_borrow_mut(Option::take);
    }
}
