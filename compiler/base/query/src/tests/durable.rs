//! Durability: a memo reading shared inputs alone holds at once in another database whose shared
//! inputs have the fingerprint it held under, its reads never verified; any other memo, or a
//! shared input changed, is verified as usual.

use crate::{Cx, Db, Event, Query, Reuse};

/// A file's text, an input.
struct Text;
impl Query for Text {
    const NAME: &'static str = "text";
    const INPUT: bool = true;
    type Key = u32;
    type Value = String;
    fn compute(_: &Cx<'_>, _: &u32) -> String {
        String::new()
    }
}

/// A file's length.
struct Length;
impl Query for Length {
    const NAME: &'static str = "length";
    type Key = u32;
    type Value = usize;
    fn compute(cx: &Cx<'_>, key: &u32) -> usize {
        cx.get::<Text>(key).len()
    }
}

/// The lengths of the files below `key`, added.
struct Total;
impl Query for Total {
    const NAME: &'static str = "total";
    type Key = u32;
    type Value = usize;
    fn compute(cx: &Cx<'_>, key: &u32) -> usize {
        (0..*key).map(|i| cx.get::<Length>(&i)).sum()
    }
}

/// A database reusing memos as `reuse` says, whose files `0..4` hold `text`, shared unless `own`
/// names the one that is not.
fn files(reuse: Reuse, text: &str, own: Option<u32>) -> Db {
    let db = Db::reusing(reuse);
    db.trace::<Text>();
    db.persist::<Length>();
    db.persist::<Total>();
    for i in 0..4 {
        if own == Some(i) { db.set::<Text>(&i, text.into()) } else { db.share::<Text>(&i, text.into()) }
    }
    db
}

/// What loading `saved` into `db` and asking for the total did.
fn total(db: &Db, saved: &[u8]) -> (usize, u64, u64) {
    db.load(saved.to_vec()).unwrap();
    let total = db.get::<Total>(&4);
    let tally = db.tally();
    (total, tally.total(Event::Durable), tally.total(Event::Restored))
}

#[test]
fn a_memo_reading_shared_inputs_alone_holds_at_once() {
    let first = files(Reuse::Trust, "abc", None);
    assert_eq!(first.get::<Total>(&4), 12);
    let saved = first.save();
    // The total holds at once: the lengths it read are never restored.
    assert_eq!(total(&files(Reuse::Trust, "abc", None), &saved), (12, 1, 1));
}

#[test]
fn a_shared_input_changed_verifies_as_usual() {
    let first = files(Reuse::Trust, "abc", None);
    first.get::<Total>(&4);
    let saved = first.save();
    let (sum, durable, _) = total(&files(Reuse::Trust, "abcd", None), &saved);
    assert_eq!((sum, durable), (16, 0));
}

#[test]
fn a_memo_reading_an_own_input_verifies_as_usual() {
    let first = files(Reuse::Trust, "abc", Some(2));
    first.get::<Total>(&4);
    let saved = first.save();
    let db = files(Reuse::Trust, "abc", Some(2));
    let (sum, _, restored) = total(&db, &saved);
    // Three lengths read shared files alone and hold at once; the total and the fourth do not.
    assert_eq!((sum, db.tally().count("length", Event::Durable), restored), (12, 3, 5));
}

#[test]
fn what_holds_at_once_is_checked() {
    let first = files(Reuse::Trust, "abc", None);
    first.get::<Total>(&4);
    let saved = first.save();
    assert_eq!(total(&files(Reuse::Check, "abc", None), &saved).0, 12);
}

/// The target chosen, an input shared among databases choosing alike.
struct Target;
impl Query for Target {
    const NAME: &'static str = "target";
    const INPUT: bool = true;
    type Key = ();
    type Value = u32;
    fn compute(_: &Cx<'_>, (): &()) -> u32 {
        1
    }
}

/// A file's length in units of the target's word.
struct Words;
impl Query for Words {
    const NAME: &'static str = "words";
    type Key = u32;
    type Value = usize;
    fn compute(cx: &Cx<'_>, key: &u32) -> usize {
        cx.get::<Length>(key) / cx.get::<Target>(&()).max(1) as usize
    }
}

/// A database with files `0..4` holding `abcd`, and `target` chosen.
fn targeted(target: u32) -> Db {
    let db = files(Reuse::Trust, "abcd", None);
    db.persist::<Words>();
    db.trace::<Target>();
    db.set_as::<Target>(&(), target, crate::Sharing::Chosen);
    db
}

#[test]
fn a_memo_holds_whatever_is_chosen_unless_it_reads_it() {
    let first = targeted(2);
    assert_eq!((first.get::<Total>(&4), first.get::<Words>(&0)), (16, 2));
    let saved = first.save();
    // Another target: the total reads the files alone and holds at once; the words do not.
    let second = targeted(4);
    second.load(saved).unwrap();
    assert_eq!((second.get::<Total>(&4), second.get::<Words>(&0)), (16, 1));
    let tally = second.tally();
    assert_eq!((tally.count("total", Event::Durable), tally.count("words", Event::Durable)), (1, 0));
}

#[test]
fn an_input_not_set_stays_out_of_the_fingerprint() {
    // The first database sets the target to the value it stands for unset; the second leaves it.
    let first = targeted(1);
    assert_eq!(first.get::<Words>(&0), 4);
    let saved = first.save();
    let second = files(Reuse::Trust, "abcd", None);
    second.persist::<Words>();
    second.trace::<Target>();
    second.load(saved).unwrap();
    assert_eq!(second.get::<Words>(&0), 4);
    // The words read the target set, which the second database's fingerprint leaves out: they
    // are verified as usual. The lengths read the files alone and hold at once.
    let tally = second.tally();
    assert_eq!((tally.count("words", Event::Durable), tally.count("length", Event::Durable)), (0, 1));
}
