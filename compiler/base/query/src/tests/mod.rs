use std::sync::atomic::{AtomicUsize, Ordering};

mod all;
mod durable;
mod persist;
mod reuse;
mod settle;
mod shared;

use super::{Cx, Cycle, Db, Query, Reuse, Revision};

/// Counts how many times a query ran.
pub(super) struct Runs(AtomicUsize);

impl Runs {
    const fn new() -> Self {
        Self(AtomicUsize::new(0))
    }

    fn tick(&self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }

    fn count(&self) -> usize {
        self.0.load(Ordering::Relaxed)
    }
}

static LENGTHS: Runs = Runs::new();
static PARITIES: Runs = Runs::new();

/// An input: a file's text by name.
struct Text;
impl Query for Text {
    const NAME: &'static str = "text";
    const INPUT: bool = true;
    type Key = &'static str;
    type Value = String;
    fn compute(_: &Cx<'_>, _: &Self::Key) -> String {
        String::new()
    }
}

struct Length;
impl Query for Length {
    const NAME: &'static str = "length";
    type Key = &'static str;
    type Value = usize;
    fn compute(cx: &Cx<'_>, key: &Self::Key) -> usize {
        LENGTHS.tick();
        cx.get::<Text>(key).len()
    }
}

struct Parity;
impl Query for Parity {
    const NAME: &'static str = "parity";
    type Key = &'static str;
    type Value = bool;
    fn compute(cx: &Cx<'_>, key: &Self::Key) -> bool {
        PARITIES.tick();
        cx.get::<Length>(key).is_multiple_of(2)
    }
}

#[test]
fn results_are_memoized_and_cut_off_early() {
    let db = Db::reusing(Reuse::Trust);
    db.set::<Text>(&"a", "four".into());
    assert!(db.get::<Parity>(&"a"));
    assert!(db.get::<Parity>(&"a"));
    assert_eq!((LENGTHS.count(), PARITIES.count()), (1, 1), "memoized");

    // Same length: the length is recomputed, the parity is not.
    db.set::<Text>(&"a", "five".into());
    assert!(db.get::<Parity>(&"a"));
    assert_eq!((LENGTHS.count(), PARITIES.count()), (2, 1), "cut off at an unchanged interface");

    // A different length reaches the parity.
    db.set::<Text>(&"a", "three".into());
    assert!(!db.get::<Parity>(&"a"));
    assert_eq!((LENGTHS.count(), PARITIES.count()), (3, 2));
}

#[test]
fn setting_an_equal_input_starts_no_revision() {
    let db = Db::reusing(Reuse::Trust);
    db.set::<Text>(&"b", "x".into());
    let revision = db.revision();
    db.set::<Text>(&"b", "x".into());
    assert_eq!(db.revision(), revision);
    db.set::<Text>(&"b", "y".into());
    assert_eq!(db.revision(), Revision(revision.0 + 1));
    assert_eq!(db.get::<Text>(&"b"), "y");
    assert_eq!(db.get::<Text>(&"unset"), "", "an unset input is computed");
}

/// Reads the next key modulo 3, so that every key reaches itself.
struct Ring;
impl Query for Ring {
    const NAME: &'static str = "ring";
    type Key = u8;
    type Value = Vec<(String, String)>;
    fn compute(cx: &Cx<'_>, key: &u8) -> Self::Value {
        cx.get::<Self>(&((key + 1) % 3))
    }
    fn cycle(_: &Cx<'_>, _: &u8, cycle: &Cycle) -> Self::Value {
        cycle.path.iter().map(|(name, key)| ((*name).to_owned(), key.clone())).collect()
    }
}

#[test]
fn cycles_are_named_and_recovered() {
    let db = Db::reusing(Reuse::Trust);
    let path: Vec<_> = db.get::<Ring>(&0).into_iter().map(|(_, key)| key).collect();
    assert_eq!(path, ["0", "1", "2", "0"]);
    let cycle = Cycle { path: vec![("ring", "0".into()), ("ring", "1".into())] };
    assert_eq!(cycle.to_string(), "query cycle: ring(0) then ring(1)");
}

#[test]
fn memos_reading_a_cycle_are_verified_in_a_later_revision() {
    let db = Db::reusing(Reuse::Trust);
    db.get::<Ring>(&0);
    // A new revision: verifying the ring reaches its first memo again, and recovers as computing.
    db.set::<Text>(&"c", "x".into());
    db.set::<Text>(&"c", "y".into());
    let path = db.get::<Ring>(&0);
    assert_eq!(path.len(), 4);
    assert_eq!(path.first(), path.last(), "a cycle");
}

static PARTS: Runs = Runs::new();

/// The parts a type reaches, itself included: a join over what it holds, where a type reached
/// again adds nothing, as the kinds of recursive types are computed.
struct Parts;
impl Query for Parts {
    const NAME: &'static str = "parts";
    type Key = u8;
    type Value = Vec<u8>;
    fn compute(cx: &Cx<'_>, key: &u8) -> Vec<u8> {
        PARTS.tick();
        // 0 holds 1, 1 holds 0 and 2.
        let holds: &[u8] = match key {
            0 => &[1],
            1 => &[0, 2],
            _ => &[],
        };
        let mut parts = vec![*key];
        parts.extend(holds.iter().flat_map(|part| cx.get::<Self>(part)));
        parts.sort_unstable();
        parts.dedup();
        parts
    }
    fn cycle(_: &Cx<'_>, _: &u8, _: &Cycle) -> Vec<u8> {
        Vec::new()
    }
}

#[test]
fn a_value_computed_from_a_cycle_value_is_not_kept() {
    let db = Db::reusing(Reuse::Trust);
    assert_eq!(db.get::<Parts>(&0), [0, 1, 2]);
    // Computing 0 read 1 through 0's cycle value, which lacked 0: 1 is computed again on its own.
    let runs = PARTS.count();
    assert_eq!(db.get::<Parts>(&1), [0, 1, 2]);
    assert!(PARTS.count() > runs);
    assert_eq!(db.get::<Parts>(&1), [0, 1, 2]);
    assert_eq!(db.get::<Parts>(&2), [2]);
}

#[test]
fn resources_are_shared_per_type() {
    let db = Db::reusing(Reuse::Trust);
    let first = db.resource::<std::sync::Mutex<Vec<u8>>>();
    first.lock().unwrap().push(1);
    assert_eq!(*db.resource::<std::sync::Mutex<Vec<u8>>>().lock().unwrap(), [1]);
}

static SLOW: Runs = Runs::new();

/// A query slow enough that threads asking for it meet while it runs.
struct Slow;
impl Query for Slow {
    const NAME: &'static str = "slow";
    type Key = u8;
    type Value = u8;
    fn compute(_: &Cx<'_>, key: &u8) -> u8 {
        SLOW.tick();
        std::thread::sleep(std::time::Duration::from_millis(50));
        key + 1
    }
}

#[test]
fn threads_needing_one_memo_compute_it_once() {
    let db = Db::reusing(Reuse::Trust);
    std::thread::scope(|s| {
        for _ in 0..8 {
            s.spawn(|| assert_eq!(db.get::<Slow>(&1), 2));
        }
    });
    assert_eq!(SLOW.count(), 1);
}

/// Where both threads of the cycle below meet before reading the other half.
static MEET: std::sync::Barrier = std::sync::Barrier::new(2);
static MET: std::sync::Once = std::sync::Once::new();

/// Meets the other thread the first time, then reads `Q`.
fn meet_then<Q: Query<Key = u8, Value = u8>>(cx: &Cx<'_>, key: u8) -> u8 {
    if !MET.is_completed() {
        MEET.wait();
        MET.call_once(|| {});
    }
    cx.get::<Q>(&key)
}

/// A cycle through two queries, each thread starting from one of them.
struct Ping;
struct Pong;
impl Query for Ping {
    const NAME: &'static str = "ping";
    type Key = u8;
    type Value = u8;
    fn compute(cx: &Cx<'_>, key: &u8) -> u8 {
        meet_then::<Pong>(cx, *key)
    }
    fn cycle(_: &Cx<'_>, _: &u8, _: &Cycle) -> u8 {
        0
    }
}
impl Query for Pong {
    const NAME: &'static str = "pong";
    type Key = u8;
    type Value = u8;
    fn compute(cx: &Cx<'_>, key: &u8) -> u8 {
        meet_then::<Ping>(cx, *key)
    }
    fn cycle(_: &Cx<'_>, _: &u8, _: &Cycle) -> u8 {
        0
    }
}

#[test]
fn a_cycle_across_threads_ends_without_waiting_forever() {
    let db = Db::reusing(Reuse::Trust);
    std::thread::scope(|s| {
        s.spawn(|| db.get::<Ping>(&1));
        s.spawn(|| db.get::<Pong>(&1));
    });
    assert_eq!(db.get::<Ping>(&1), 0);
}

static FLAKY: Runs = Runs::new();

/// Fails the first time it runs; a cycle through it would give `None`.
struct Flaky;
impl Query for Flaky {
    const NAME: &'static str = "flaky";
    type Key = u8;
    type Value = Option<u8>;
    fn compute(_: &Cx<'_>, key: &u8) -> Option<u8> {
        FLAKY.tick();
        assert!(FLAKY.count() > 1, "the first run fails");
        Some(*key)
    }
    fn cycle(_: &Cx<'_>, _: &u8, _: &Cycle) -> Option<u8> {
        None
    }
}

#[test]
fn a_panic_in_a_computation_leaves_no_frame_behind() {
    let db = Db::reusing(Reuse::Trust);
    let cx = Cx::new(&db);
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cx.get::<Flaky>(&7)));
    assert!(caught.is_err());
    assert_eq!(cx.get::<Flaky>(&7), Some(7), "computed again, not taken for a cycle");
}
