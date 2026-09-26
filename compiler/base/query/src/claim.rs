//! Claims of memos being computed, so that one thread computes a memo while the others needing it
//! wait. A thread about to wait on another first follows who that one waits on: if the chain comes
//! back to it, waiting would never end, and it computes the memo itself. Two threads may then
//! compute the same memo, which is only work done twice: a query's value depends on its key and
//! inputs alone.

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, ThreadId};

/// The claim on one memo: the computation running it, if any.
#[derive(Default)]
pub(crate) struct Claims(Mutex<Option<Arc<Running>>>);

/// A computation running, and the signal that it is done.
struct Running {
    owner: ThreadId,
    done: Mutex<bool>,
    finished: Condvar,
}

/// What computing a memo other threads may need takes.
pub(crate) enum Claim<'c> {
    /// This thread computes it; others wait until the guard is dropped.
    Owner(Owned<'c>),
    /// This thread computes it without telling others: waiting would never end.
    Alone,
    /// Another thread computed it meanwhile, or gave up: look again.
    Waited,
}

/// A memo claimed, released when dropped, a panic included.
pub(crate) struct Owned<'c> {
    claims: &'c Claims,
    running: Arc<Running>,
}

/// Which thread each waiting thread waits on. A lock: checking a chain of waits and adding one
/// must be one step, or two threads could each start waiting on the other. Only threads about to
/// wait take it.
#[derive(Default)]
pub(crate) struct Waits(Mutex<HashMap<ThreadId, ThreadId>>);

impl Claims {
    /// Claims the memo to compute it, or waits for the thread computing it.
    pub(crate) fn claim<'c>(&'c self, waits: &Waits) -> Claim<'c> {
        let me = thread::current().id();
        let other = {
            let mut current = self.0.lock().expect("a claim");
            if let Some(running) = &*current {
                Arc::clone(running)
            } else {
                let running =
                    Arc::new(Running { owner: me, done: Mutex::new(false), finished: Condvar::new() });
                *current = Some(Arc::clone(&running));
                return Claim::Owner(Owned { claims: self, running });
            }
        };
        if other.owner == me || !waits.start(me, other.owner) {
            return Claim::Alone;
        }
        let done = other.done.lock().expect("a running computation");
        drop(other.finished.wait_while(done, |done| !*done).expect("a running computation"));
        waits.end(me);
        Claim::Waited
    }
}

impl Drop for Owned<'_> {
    fn drop(&mut self) {
        *self.claims.0.lock().expect("a claim") = None;
        *self.running.done.lock().expect("a running computation") = true;
        self.running.finished.notify_all();
    }
}

impl Waits {
    /// Records that `me` waits on `owner`, unless `owner` waits, through others, on `me`.
    fn start(&self, me: ThreadId, owner: ThreadId) -> bool {
        let mut waits = self.0.lock().expect("waits");
        let mut at = owner;
        while let Some(&next) = waits.get(&at) {
            if next == me {
                return false;
            }
            at = next;
        }
        waits.insert(me, owner);
        true
    }

    fn end(&self, me: ThreadId) {
        self.0.lock().expect("waits").remove(&me);
    }
}
