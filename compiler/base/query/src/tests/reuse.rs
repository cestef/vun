//! Checking reuse: a value whose fingerprint misses part of it is cut off early, and the memo
//! reused behind it is caught when computed again.

use std::hash::{Hash, Hasher};
use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::{Cx, Db, Differs, Query, Reuse, Unsound};

/// An input: a text by name.
struct Line;
impl Query for Line {
    const NAME: &'static str = "line";
    const INPUT: bool = true;
    type Key = &'static str;
    type Value = String;
    fn compute(_: &Cx<'_>, _: &Self::Key) -> String {
        String::new()
    }
}

/// Where a character is in a text, hashed by the character alone: what a fingerprint missing a
/// place looks like.
#[derive(Clone, Debug, PartialEq)]
struct Place {
    at: usize,
    name: char,
}

impl Hash for Place {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

/// The first `x` of a line.
struct Found;
impl Query for Found {
    const NAME: &'static str = "found";
    type Key = &'static str;
    type Value = Option<Place>;
    fn compute(cx: &Cx<'_>, key: &Self::Key) -> Option<Place> {
        cx.get::<Line>(key).find('x').map(|at| Place { at, name: 'x' })
    }
}

/// What follows the first `x`, by its place.
struct After;
impl Query for After {
    const NAME: &'static str = "after";
    type Key = &'static str;
    type Value = Option<usize>;
    fn compute(cx: &Cx<'_>, key: &Self::Key) -> Option<usize> {
        cx.get::<Found>(key).map(|place| place.at + 1)
    }
}

/// `After` of a line set to `first`, then to `second`, under `reuse`.
fn after(reuse: Reuse, first: &str, second: &str) -> Option<usize> {
    let db = Db::reusing(reuse);
    db.set::<Line>(&"l", first.into());
    db.get::<After>(&"l");
    db.set::<Line>(&"l", second.into());
    db.get::<After>(&"l")
}

#[test]
fn a_memo_reused_behind_a_blind_fingerprint_is_caught_when_checked() {
    // Trusted, the stale place is reused: the `x` moved, yet the answer is the old one.
    assert_eq!(after(Reuse::Trust, "ax", "bbx"), Some(2));
    let caught = catch_unwind(AssertUnwindSafe(|| after(Reuse::Check, "ax", "bbx"))).unwrap_err();
    let expected = Unsound { query: After::NAME, key: format!("{:?}", "l"), differs: Differs::Fingerprint };
    assert_eq!(caught.downcast_ref::<String>(), Some(&expected.to_string()));
}

#[test]
fn a_sound_reuse_passes_the_check() {
    assert_eq!(after(Reuse::Check, "ax", "bx"), Some(2));
}
