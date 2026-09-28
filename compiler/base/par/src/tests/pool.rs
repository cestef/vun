//! The pool's `unsafe` lending, tested for what makes it sound: the work, borrowing the caller's
//! stack, is never used after the call ends, however it ends; every result is dropped once; and
//! a call never waits on work it did not lend. Run under Miri as well (`cargo +nightly miri test
//! -p vun-par`), which checks every access against the borrow's lifetime.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use crate::Parallel;

/// How many times a test repeats: fewer under Miri, which interprets every step.
const ROUNDS: usize = if cfg!(miri) { 4 } else { 200 };

#[test]
fn work_borrows_the_callers_stack_and_is_done_with_it_by_the_end() {
    for round in 0..ROUNDS {
        let words: Vec<String> = (0..64).map(|i| format!("{round}-{i}")).collect();
        let suffix = String::from("!");
        let joined = words.par_map(|w| format!("{w}{suffix}"));
        assert_eq!(joined, words.iter().map(|w| format!("{w}!")).collect::<Vec<_>>());
        // `words` and `suffix` go here, which a worker still holding the work would read after.
    }
}

/// A value counting how many of its kind were made and dropped.
struct Counted<'a> {
    dropped: &'a AtomicUsize,
}

impl Drop for Counted<'_> {
    fn drop(&mut self) {
        self.dropped.fetch_add(1, Ordering::Relaxed);
    }
}

#[test]
fn every_result_is_dropped_once_whether_the_call_ends_or_panics() {
    let (made, dropped) = (AtomicUsize::new(0), AtomicUsize::new(0));
    let items: Vec<usize> = (0..256).collect();
    let counted = |_: &usize| {
        made.fetch_add(1, Ordering::Relaxed);
        Counted { dropped: &dropped }
    };
    drop(items.par_map(counted));
    assert_eq!(made.load(Ordering::Relaxed), 256);
    assert_eq!(dropped.load(Ordering::Relaxed), 256);

    let failing = std::panic::catch_unwind(|| {
        items.par_map(|&n| {
            assert_ne!(n, 100);
            counted(&n)
        })
    });
    assert!(failing.is_err());
    assert_eq!(made.load(Ordering::Relaxed), dropped.load(Ordering::Relaxed), "none leaked, none twice");
}

#[test]
fn a_call_that_panics_returns_only_once_no_worker_runs_its_work() {
    for _ in 0..ROUNDS.min(20) {
        let (started, finished) = (AtomicUsize::new(0), AtomicUsize::new(0));
        let items: Vec<usize> = (0..64).collect();
        let failing = std::panic::catch_unwind(|| {
            items.par_map(|&n| {
                started.fetch_add(1, Ordering::SeqCst);
                assert_ne!(n, 0, "the first item fails at once");
                thread::sleep(Duration::from_micros(200));
                finished.fetch_add(1, Ordering::SeqCst);
            })
        });
        assert!(failing.is_err());
        // Every item begun but the one that failed has ended: no worker is inside the work.
        assert_eq!(started.load(Ordering::SeqCst), finished.load(Ordering::SeqCst) + 1);
    }
}

#[test]
fn many_callers_at_once_each_get_their_own_results() {
    let callers = if cfg!(miri) { 3 } else { 16 };
    thread::scope(|scope| {
        for caller in 0..callers {
            scope.spawn(move || {
                for round in 0..ROUNDS.min(50) {
                    let items: Vec<usize> = (0..97).map(|i| i + caller * 1000 + round).collect();
                    let doubled = items.par_map(|n| n * 2);
                    assert_eq!(doubled, items.iter().map(|n| n * 2).collect::<Vec<_>>());
                }
            });
        }
    });
}

#[test]
fn nested_calls_from_many_callers_run_on_their_workers() {
    thread::scope(|scope| {
        for _ in 0..if cfg!(miri) { 2 } else { 8 } {
            scope.spawn(|| {
                let rows: Vec<u64> = (0..32).collect();
                let sums =
                    rows.par_map(|&r| (0..50).collect::<Vec<u64>>().par_map(|c| r * c).iter().sum::<u64>());
                assert_eq!(sums, rows.iter().map(|r| r * 1225).collect::<Vec<_>>());
            });
        }
    });
}

#[test]
fn a_call_while_every_worker_is_busy_runs_without_waiting_for_them() {
    let (release, busy) = (AtomicBool::new(false), AtomicUsize::new(0));
    let workers = crate::threads();
    thread::scope(|scope| {
        // Holds every thread of the pool until released.
        let holder = scope.spawn(|| {
            let items: Vec<usize> = (0..workers * 2).collect();
            items.par_map(|_| {
                busy.fetch_add(1, Ordering::SeqCst);
                while !release.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(1));
                }
            });
        });
        // At least the holder's own thread; whatever workers were idle then are held too, and
        // other tests running at once may hold the rest.
        while busy.load(Ordering::SeqCst) == 0 {
            thread::sleep(Duration::from_millis(1));
        }
        // Were this call to wait on a worker held above, it would never end: the release is after.
        let items: Vec<usize> = (0..100).collect();
        assert_eq!(items.par_map(|n| n + 1).len(), 100);
        release.store(true, Ordering::SeqCst);
        holder.join().unwrap();
    });
}
