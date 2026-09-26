//! One query's memos, shared by every thread without locks: reading a memo is a lookup in a
//! concurrent map, and a key being computed is claimed in another, so that one thread computes it
//! while the others needing it wait.
//!
//! A thread about to wait on another first follows who that one waits on: if the chain comes back
//! to it, waiting would never end, and it computes the memo itself. Two threads may then compute
//! the same memo, which is only work done twice: a query's value depends on its key and inputs
//! alone.

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, ThreadId};

use vun_hash::Hash;

use super::memo::Memo;
use super::{Query, Revision};

/// A map every thread reads and writes at once, hashed fast.
pub type Map<K, V> = papaya::HashMap<K, V, foldhash::fast::RandomState>;

pub(crate) struct Table<Q: Query> {
    memos: Map<Q::Key, Arc<Memo<Q>>>,
    /// The keys being computed, with the thread computing each.
    running: Map<Q::Key, Arc<Running>>,
}

/// A memo found: current, or to verify.
pub(crate) enum Lookup<Q: Query> {
    Current(Q::Value, Hash),
    Stale(Arc<Memo<Q>>),
}

/// A memo being computed, and the signal that it is done.
pub(crate) struct Running {
    owner: ThreadId,
    done: Mutex<bool>,
    finished: Condvar,
}

/// What computing a memo other threads may need takes.
pub(crate) enum Claim<'t, Q: Query> {
    /// This thread computes it; others wait until the guard is dropped.
    Owner(Owned<'t, Q>),
    /// This thread computes it without telling others: waiting would never end.
    Alone,
    /// Another thread computed it meanwhile, or gave up: look again.
    Waited,
}

/// The claim of a key being computed, released when dropped, a panic included.
pub(crate) struct Owned<'t, Q: Query> {
    table: &'t Table<Q>,
    key: Q::Key,
    running: Arc<Running>,
}

/// Which thread each waiting thread waits on. A lock, unlike the tables: checking a chain of
/// waits and adding one must be one step, or two threads could each start waiting on the other.
/// Only threads about to wait take it.
#[derive(Default)]
pub(crate) struct Waits(Mutex<HashMap<ThreadId, ThreadId>>);

impl<Q: Query> Default for Table<Q> {
    fn default() -> Self {
        Self { memos: Map::default(), running: Map::default() }
    }
}

impl<Q: Query> Table<Q> {
    pub(crate) fn get(&self, key: &Q::Key) -> Option<Arc<Memo<Q>>> {
        self.memos.pin().get(key).cloned()
    }

    /// The memo of `key` as `revision` finds it: its value when it is current, taken while the
    /// map is pinned, else the memo itself, to be verified.
    pub(crate) fn lookup(&self, key: &Q::Key, revision: Revision) -> Option<Lookup<Q>> {
        let memos = self.memos.pin();
        let memo = memos.get(key)?;
        Some(if memo.current(revision) {
            Lookup::Current(memo.value.clone(), memo.hash)
        } else {
            Lookup::Stale(Arc::clone(memo))
        })
    }

    pub(crate) fn insert(&self, key: Q::Key, memo: Arc<Memo<Q>>) {
        self.memos.pin().insert(key, memo);
    }

    pub(crate) fn all(&self) -> Vec<(Q::Key, Arc<Memo<Q>>)> {
        self.memos.pin().iter().map(|(key, memo)| (key.clone(), Arc::clone(memo))).collect()
    }

    /// Claims `key` to compute it, or waits for the thread computing it.
    pub(crate) fn claim(&self, key: &Q::Key, waits: &Waits) -> Claim<'_, Q> {
        let me = thread::current().id();
        let mine = Arc::new(Running { owner: me, done: Mutex::new(false), finished: Condvar::new() });
        let other = match self.running.pin().try_insert(key.clone(), Arc::clone(&mine)) {
            Ok(_) => return Claim::Owner(Owned { table: self, key: key.clone(), running: mine }),
            Err(occupied) => Arc::clone(occupied.current),
        };
        if other.owner == me || !waits.start(me, other.owner) {
            return Claim::Alone;
        }
        let done = other.done.lock().expect("a running query");
        drop(other.finished.wait_while(done, |done| !*done).expect("a running query"));
        waits.end(me);
        Claim::Waited
    }
}

impl<Q: Query> Drop for Owned<'_, Q> {
    fn drop(&mut self) {
        self.table.running.pin().remove(&self.key);
        *self.running.done.lock().expect("a running query") = true;
        self.running.finished.notify_all();
    }
}

impl Waits {
    /// Records that `me` waits on `owner`, unless `owner` waits, through others, on `me`.
    fn start(&self, me: ThreadId, owner: ThreadId) -> bool {
        let mut waits = self.0.lock().expect("query waits");
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
        self.0.lock().expect("query waits").remove(&me);
    }
}
