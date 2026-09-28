//! Independent work spread over threads, results in the items' order, and the map threads share
//! state in.
//!
//! The caller works alongside the pool's threads, started once for the process (`pool`). Items are claimed in runs from one shared
//! counter, guided: a run is a share of what is left, so early claims are long and cheap to
//! coordinate and the last ones single items that balance uneven costs. Each worker keeps its
//! runs' results together; they are joined in order at the end. A call made from inside a worker
//! runs on that worker alone: the machine is already busy, and more threads would only compete.

use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock, PoisonError};
use std::thread;

mod pool;

use pool::Pool;

#[cfg(test)]
mod tests;

/// The stack of each thread: as large as a process's main thread's, since compile-time
/// evaluation recurses deeply.
pub(crate) const STACK: usize = 8 << 20;

/// How many runs each worker takes of what is left: a run is `left / (SHARES * workers)` items.
const SHARES: usize = 2;

/// The claim counter, on a cache line of its own so that claims do not slow what sits beside it.
#[repr(align(128))]
struct Counter(AtomicUsize);

thread_local! {
    /// Whether this thread is working for a `par_map`.
    static WORKING: Cell<bool> = const { Cell::new(false) };
}

/// This thread marked as working while it lives, even through a panic, then as it was before.
pub(crate) struct Working(bool);

impl Working {
    pub(crate) fn start() -> Self {
        Self(WORKING.replace(true))
    }
}

impl Drop for Working {
    fn drop(&mut self) {
        WORKING.set(self.0);
    }
}

/// A map every thread reads and writes at once without locks, hashed fast: what state shared by
/// threads is kept in.
pub type Map<K, V> = papaya::HashMap<K, V, foldhash::fast::RandomState>;

/// Items a function maps in parallel.
pub trait Parallel<T> {
    /// `f` of each item, computed on as many threads as the machine runs at once, in order.
    fn par_map<R: Send>(&self, f: impl Fn(&T) -> R + Sync) -> Vec<R>;
}

impl<T: Sync> Parallel<T> for [T] {
    fn par_map<R: Send>(&self, f: impl Fn(&T) -> R + Sync) -> Vec<R> {
        let workers = threads().min(self.len());
        if workers <= 1 || WORKING.get() {
            return self.iter().map(f).collect();
        }
        let counter = Counter(AtomicUsize::new(0));
        let runs: Mutex<Vec<(usize, Vec<R>)>> = Mutex::new(Vec::new());
        let work = || {
            let _working = Working::start();
            let mut mine = Vec::new();
            while let Some((start, end)) = claim(&counter.0, self.len(), workers) {
                mine.push((start, self[start..end].iter().map(&f).collect::<Vec<R>>()));
            }
            runs.lock().unwrap_or_else(PoisonError::into_inner).extend(mine);
        };
        Pool::global().run(workers - 1, &work);
        let mut runs = runs.into_inner().unwrap_or_else(PoisonError::into_inner);
        runs.sort_unstable_by_key(|&(start, _)| start);
        let mut all = Vec::with_capacity(self.len());
        for (_, results) in runs {
            all.extend(results);
        }
        all
    }
}

/// The next run of items `start..end` out of `len`, shared among `workers`; `None` when all are
/// taken.
fn claim(counter: &AtomicUsize, len: usize, workers: usize) -> Option<(usize, usize)> {
    let mut start = counter.load(Ordering::Relaxed);
    loop {
        if start >= len {
            return None;
        }
        let end = start + ((len - start) / (SHARES * workers)).max(1);
        match counter.compare_exchange_weak(start, end, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return Some((start, end)),
            Err(now) => start = now,
        }
    }
}

/// How many threads the machine runs at once, asked once.
pub(crate) fn threads() -> usize {
    static THREADS: OnceLock<usize> = OnceLock::new();
    *THREADS.get_or_init(|| thread::available_parallelism().map_or(1, std::num::NonZero::get))
}
