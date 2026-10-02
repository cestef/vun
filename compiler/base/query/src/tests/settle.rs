//! Verifying ahead: the reads of a memo reading many are found to hold over threads, computing
//! none, so that only what changed is computed again, and never a read an earlier one no longer
//! names.

use super::Runs;
use crate::{Cx, Db, Query, Reuse};

/// How many items the total adds, an input.
struct Count;
impl Query for Count {
    const NAME: &'static str = "count";
    const INPUT: bool = true;
    type Key = ();
    type Value = u32;
    fn compute(_: &Cx<'_>, (): &()) -> u32 {
        0
    }
}

/// A number, an input.
struct Number;
impl Query for Number {
    const NAME: &'static str = "number";
    const INPUT: bool = true;
    type Key = u32;
    type Value = u64;
    fn compute(_: &Cx<'_>, key: &u32) -> u64 {
        u64::from(*key)
    }
}

/// A number doubled.
struct Item;
impl Query for Item {
    const NAME: &'static str = "item";
    type Key = u32;
    type Value = u64;
    fn compute(cx: &Cx<'_>, key: &u32) -> u64 {
        Runs::<Self>::tick(cx);
        cx.get::<Number>(key) * 2
    }
}

/// The first `Count` items, added: each read of an item is keyed by the count read before it.
struct Total;
impl Query for Total {
    const NAME: &'static str = "total";
    type Key = ();
    type Value = u64;
    fn compute(cx: &Cx<'_>, (): &()) -> u64 {
        Runs::<Self>::tick(cx);
        (0..cx.get::<Count>(&())).map(|i| cx.get::<Item>(&i)).sum()
    }
}

/// The total of the items below `count`, their numbers as `Number` computes them but `changed`.
fn total(count: u32, changed: (u32, u64)) -> u64 {
    (0..count).map(|i| if i == changed.0 { changed.1 } else { u64::from(i) } * 2).sum()
}

fn settles(reuse: Reuse) {
    let db = Db::reusing(reuse);
    db.set::<Count>(&(), 40);
    assert_eq!(db.get::<Total>(&()), total(40, (0, 0)));
    let (items, totals) = (Runs::<Item>::of(&db), Runs::<Total>::of(&db));
    // One number of forty changes: its item is computed again, the other items hold.
    db.set::<Number>(&7, 100);
    assert_eq!(db.get::<Total>(&()), total(40, (7, 100)));
    assert!(Runs::<Total>::of(&db) > totals, "the total is computed again");
    if reuse == Reuse::Trust {
        assert_eq!(Runs::<Item>::of(&db), items + 1, "one item computed again");
    }
    // Fewer items: the total is computed again from the count it reads first.
    db.set::<Count>(&(), 20);
    assert_eq!(db.get::<Total>(&()), total(20, (7, 100)));
}

#[test]
fn only_what_changed_is_computed_again() {
    settles(Reuse::Trust);
}

#[test]
fn what_verifying_ahead_finds_is_checked() {
    settles(Reuse::Check);
}
