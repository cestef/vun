//! Many keys of a query computed at once, over threads, and read as the caller's reads.

use super::Runs;
use crate::{Cx, Db, Query, Reuse};

static SUMS: Runs = Runs::new();

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

/// A number squared.
struct Square;
impl Query for Square {
    const NAME: &'static str = "square";
    type Key = u32;
    type Value = u64;
    fn compute(cx: &Cx<'_>, key: &u32) -> u64 {
        let n = cx.get::<Number>(key);
        n * n
    }
}

/// The squares of the numbers below `key`, added, computed through `get_all`.
struct Sum;
impl Query for Sum {
    const NAME: &'static str = "sum";
    type Key = u32;
    type Value = u64;
    fn compute(cx: &Cx<'_>, key: &u32) -> u64 {
        SUMS.tick();
        let keys: Vec<u32> = (0..*key).collect();
        cx.get_all::<Square>(&keys).into_iter().sum()
    }
}

#[test]
fn values_come_in_order_and_each_is_a_read_of_the_caller() {
    let db = Db::reusing(Reuse::Trust);
    assert_eq!(db.get::<Sum>(&100), (0..100u64).map(|n| n * n).sum::<u64>());
    let runs = SUMS.count();
    assert_eq!(db.get::<Sum>(&100), (0..100u64).map(|n| n * n).sum::<u64>());
    assert_eq!(SUMS.count(), runs, "reused");
    // A number read through one of the squares changes the sum.
    db.set::<Number>(&7, 0);
    assert_eq!(db.get::<Sum>(&100), (0..100u64).map(|n| n * n).sum::<u64>() - 49);
    assert_eq!(SUMS.count(), runs + 1);
}
