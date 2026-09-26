//! Memos saved by one database and loaded by another.

use super::Runs;
use crate::{Cx, Db, Query, Unloaded};

static WORDS: Runs = Runs::new();

/// A persisted input: a file's text by path.
struct Source;
impl Query for Source {
    const NAME: &'static str = "source";
    const INPUT: bool = true;
    type Key = String;
    type Value = String;
    fn compute(_: &Cx<'_>, _: &String) -> String {
        String::new()
    }
}

struct Words;
impl Query for Words {
    const NAME: &'static str = "words";
    type Key = String;
    type Value = Vec<String>;
    fn compute(cx: &Cx<'_>, key: &String) -> Vec<String> {
        WORDS.tick();
        cx.get::<Source>(key).split_whitespace().map(str::to_owned).collect()
    }
}

/// A database with the persisted queries registered and `text` set as the source of `a`.
fn persisting(text: &str) -> Db {
    let db = Db::new();
    db.trace::<Source>();
    db.persist::<Words>();
    db.set::<Source>(&"a".into(), text.into());
    db
}

#[test]
fn saved_memos_are_reused_until_their_trace_changes() {
    let first = persisting("one two");
    assert_eq!(first.get::<Words>(&"a".into()), ["one", "two"]);
    let saved = first.save();
    assert_eq!(saved, first.save(), "the saved form is deterministic");
    assert_eq!(WORDS.count(), 1);

    // Another process with the same input reuses the memo.
    let second = persisting("one two");
    assert_eq!(second.load(saved.clone()).unwrap(), 1, "only computed memos are saved");
    assert_eq!(second.get::<Words>(&"a".into()), ["one", "two"]);
    assert_eq!(WORDS.count(), 1);

    assert!(!second.changed(), "nothing computed that was not loaded");
    assert!(matches!(second.load(saved.clone()), Err(Unloaded::Again)));

    // A changed input fails the trace, so the loaded memo is recomputed.
    let third = persisting("three");
    third.load(saved.clone()).unwrap();
    assert_eq!(third.get::<Words>(&"a".into()), ["three"]);
    assert_eq!(WORDS.count(), 2);
    assert!(third.changed());
}

#[test]
fn values_never_read_are_saved_again_as_they_were() {
    let first = persisting("x y");
    first.set::<Source>(&"b".into(), "z".into());
    first.get::<Words>(&"a".into());
    first.get::<Words>(&"b".into());
    let saved = first.save();

    // The next process reads one memo's value and saves; the other's bytes are carried over.
    let second = persisting("x y");
    second.set::<Source>(&"b".into(), "z".into());
    second.load(saved.clone()).unwrap();
    assert_eq!(second.get::<Words>(&"a".into()), ["x", "y"]);
    let again = second.save();
    let third = persisting("x y");
    third.set::<Source>(&"b".into(), "z".into());
    third.load(again.clone()).unwrap();
    let runs = WORDS.count();
    assert_eq!(third.get::<Words>(&"b".into()), ["z"]);
    assert_eq!(WORDS.count(), runs, "read from the bytes carried over");
}

#[test]
fn memos_of_unregistered_queries_and_other_formats_are_not_loaded() {
    let first = persisting("x");
    first.get::<Words>(&"a".into());
    let saved = first.save();
    let bare = Db::new();
    bare.persist::<Words>();
    assert_eq!(bare.load(saved.clone()).unwrap(), 0, "a trace reading an unregistered query is skipped");
    // The format is the leading varint.
    let mut other = saved.clone();
    other[0] = 1;
    assert!(matches!(Db::new().load(other.clone()), Err(Unloaded::Format { found: 1 })));
    assert!(matches!(Db::new().load(Vec::new()), Err(Unloaded::Decode(_))));
}

static COUNTS: Runs = Runs::new();

/// Reads a traced query: its memos are saved without their values.
struct Count;
impl Query for Count {
    const NAME: &'static str = "count";
    type Key = String;
    type Value = usize;
    fn compute(cx: &Cx<'_>, key: &String) -> usize {
        COUNTS.tick();
        cx.get::<Tokens>(key).len()
    }
}

static TOKENS: Runs = Runs::new();

struct Tokens;
impl Query for Tokens {
    const NAME: &'static str = "tokens";
    type Key = String;
    type Value = Vec<String>;
    fn compute(cx: &Cx<'_>, key: &String) -> Vec<String> {
        TOKENS.tick();
        cx.get::<Source>(key).split_whitespace().map(str::to_owned).collect()
    }
}

fn counting(text: &str) -> Db {
    let db = Db::new();
    db.trace::<Source>();
    db.trace::<Tokens>();
    db.persist::<Count>();
    db.set::<Source>(&"b".into(), text.into());
    db
}

#[test]
fn a_traced_memo_stands_for_its_value_until_it_is_read() {
    let first = counting("x y");
    assert_eq!(first.get::<Count>(&"b".into()), 2);
    let saved = first.save();
    assert_eq!((TOKENS.count(), COUNTS.count()), (1, 1));

    let second = counting("x y");
    assert_eq!(second.load(saved.clone()).unwrap(), 2);
    assert_eq!(second.get::<Count>(&"b".into()), 2);
    assert_eq!((TOKENS.count(), COUNTS.count()), (1, 1), "verified through the traced memo");
    assert_eq!(second.get::<Tokens>(&"b".into()), ["x", "y"]);
    assert_eq!(TOKENS.count(), 2, "its value is computed once read");
}
