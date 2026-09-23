use std::sync::atomic::{AtomicUsize, Ordering};

use super::{Cx, Cycle, Db, Query, Revision};

/// Counts how many times a query ran.
struct Runs(AtomicUsize);

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
    let db = Db::new();
    db.set::<Text>("a", "four".into());
    assert!(db.get::<Parity>(&"a"));
    assert!(db.get::<Parity>(&"a"));
    assert_eq!((LENGTHS.count(), PARITIES.count()), (1, 1), "memoized");

    // Same length: the length is recomputed, the parity is not.
    db.set::<Text>("a", "five".into());
    assert!(db.get::<Parity>(&"a"));
    assert_eq!((LENGTHS.count(), PARITIES.count()), (2, 1), "cut off at an unchanged interface");

    // A different length reaches the parity.
    db.set::<Text>("a", "three".into());
    assert!(!db.get::<Parity>(&"a"));
    assert_eq!((LENGTHS.count(), PARITIES.count()), (3, 2));
}

#[test]
fn setting_an_equal_input_starts_no_revision() {
    let db = Db::new();
    db.set::<Text>("b", "x".into());
    let revision = db.revision();
    db.set::<Text>("b", "x".into());
    assert_eq!(db.revision(), revision);
    db.set::<Text>("b", "y".into());
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
    let db = Db::new();
    let path: Vec<_> = db.get::<Ring>(&0).into_iter().map(|(_, key)| key).collect();
    assert_eq!(path, ["0", "1", "2", "0"]);
    let cycle = Cycle { path: vec![("ring", "0".into()), ("ring", "1".into())] };
    assert_eq!(cycle.to_string(), "query cycle: ring(0) then ring(1)");
}
