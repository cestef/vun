//! Memos saved by one database and loaded by another.

use vun_codec::{Blob, Decode};

use super::Runs;
use crate::persist::Saved;
use crate::{Census, Cx, Db, Query, Reuse, Unloaded};

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
        Runs::<Self>::tick(cx);
        cx.get::<Source>(key).split_whitespace().map(str::to_owned).collect()
    }
}

/// A database with the persisted queries registered and `text` set as the source of `a`.
fn persisting(text: &str) -> Db {
    let db = Db::reusing(Reuse::Trust);
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
    assert_eq!(Runs::<Words>::of(&first), 1);

    // Another process with the same input reuses the memo.
    let second = persisting("one two");
    let memos = Census::of(&saved).unwrap().queries.iter().map(|q| q.memos).collect::<Vec<_>>();
    assert_eq!(memos, [0, 1], "only computed memos are saved");
    assert_eq!(second.load(saved.clone()).unwrap(), 2, "both queries are registered");
    assert_eq!(second.get::<Words>(&"a".into()), ["one", "two"]);
    assert_eq!(Runs::<Words>::of(&second), 0, "the saved memo is reused");

    assert!(!second.changed(), "nothing computed that was not loaded");
    assert!(matches!(second.load(saved.clone()), Err(Unloaded::Again)));

    // A changed input fails the trace, so the loaded memo is recomputed.
    let third = persisting("three");
    third.load(saved.clone()).unwrap();
    assert_eq!(third.get::<Words>(&"a".into()), ["three"]);
    assert_eq!(Runs::<Words>::of(&third), 1);
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
    assert_eq!(third.get::<Words>(&"b".into()), ["z"]);
    assert_eq!(Runs::<Words>::of(&third), 0, "read from the bytes carried over");
}

/// The size a saved file's body had when last written whole, and its size.
fn growth(saved: &[u8]) -> (u64, usize) {
    let header = Saved::<Blob>::decoded(saved).unwrap();
    (header.compacted, header.body.len)
}

#[test]
fn a_body_is_written_whole_once_it_outgrows_its_last_whole_size() {
    let first = persisting("p");
    first.get::<Words>(&"a".into());
    let saved = first.save();
    let (compacted, size) = growth(&saved);
    assert_eq!(compacted, size as u64, "a first save is whole");

    // A new memo past the last whole size extends the body and keeps that size.
    let second = persisting("p");
    let long = "word ".repeat(64);
    second.set::<Source>(&"b".into(), long.clone());
    second.load(saved).unwrap();
    second.get::<Words>(&"b".into());
    let grown = second.save();
    let (kept, size) = growth(&grown);
    assert_eq!(kept, compacted);
    assert!(size as u64 > 2 * compacted);

    // The next save writes it whole, reading the value never read to write it anew.
    let third = persisting("p");
    third.set::<Source>(&"b".into(), long.clone());
    third.load(grown).unwrap();
    let whole = third.save();
    let (compacted, size) = growth(&whole);
    assert_eq!(compacted, size as u64);

    let fourth = persisting("p");
    fourth.set::<Source>(&"b".into(), long);
    fourth.load(whole).unwrap();
    assert_eq!(fourth.get::<Words>(&"a".into()), ["p"]);
    assert_eq!(fourth.get::<Words>(&"b".into()).len(), 64);
    assert_eq!(Runs::<Words>::of(&fourth), 0, "both read from the body written whole");
}

#[test]
fn memos_of_unregistered_queries_and_other_formats_are_not_loaded() {
    let first = persisting("x");
    first.get::<Words>(&"a".into());
    let saved = first.save();
    let bare = Db::reusing(Reuse::Trust);
    bare.persist::<Words>();
    assert_eq!(bare.load(saved.clone()).unwrap(), 1);
    assert!(bare.get::<Words>(&"a".into()).is_empty(), "a memo reading an unregistered query is computed");
    assert_eq!(Runs::<Words>::of(&bare), 1);
    // The format is the leading varint.
    let mut other = saved.clone();
    other[0] = 1;
    assert!(matches!(Db::reusing(Reuse::Trust).load(other.clone()), Err(Unloaded::Format { found: 1 })));
    assert!(matches!(Db::reusing(Reuse::Trust).load(Vec::new()), Err(Unloaded::Decode(_))));
}

/// Reads a traced query: its memos are saved without their values.
struct Count;
impl Query for Count {
    const NAME: &'static str = "count";
    type Key = String;
    type Value = usize;
    fn compute(cx: &Cx<'_>, key: &String) -> usize {
        Runs::<Self>::tick(cx);
        cx.get::<Tokens>(key).len()
    }
}

struct Tokens;
impl Query for Tokens {
    const NAME: &'static str = "tokens";
    type Key = String;
    type Value = Vec<String>;
    fn compute(cx: &Cx<'_>, key: &String) -> Vec<String> {
        Runs::<Self>::tick(cx);
        cx.get::<Source>(key).split_whitespace().map(str::to_owned).collect()
    }
}

fn counting(text: &str) -> Db {
    let db = Db::reusing(Reuse::Trust);
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
    assert_eq!((Runs::<Tokens>::of(&first), Runs::<Count>::of(&first)), (1, 1));

    let second = counting("x y");
    assert_eq!(second.load(saved.clone()).unwrap(), 3);
    assert_eq!(second.get::<Count>(&"b".into()), 2);
    assert_eq!(
        (Runs::<Tokens>::of(&second), Runs::<Count>::of(&second)),
        (0, 0),
        "verified through the traced memo"
    );
    assert_eq!(second.get::<Tokens>(&"b".into()), ["x", "y"]);
    assert_eq!(Runs::<Tokens>::of(&second), 1, "its value is computed once read");
}

#[test]
fn a_memo_carried_over_unread_sees_the_reads_it_saw() {
    let first = counting("x y");
    first.get::<Count>(&"b".into());
    let saved = first.save();

    // The next process recomputes the tokens alone; the count it never asked for is carried over.
    let second = counting("x y z");
    second.load(saved).unwrap();
    assert_eq!(second.get::<Tokens>(&"b".into()).len(), 3);
    let carried = second.save();

    // Its read of the tokens still names the ones it saw, so it is recomputed.
    let third = counting("x y z");
    third.load(carried).unwrap();
    assert_eq!(third.get::<Count>(&"b".into()), 3);
    assert_eq!(Runs::<Count>::of(&third), 1);
}

#[test]
fn only_the_memos_asked_for_are_restored() {
    let first = persisting("x");
    first.set::<Source>(&"b".into(), "y".into());
    first.get::<Words>(&"a".into());
    first.get::<Words>(&"b".into());
    let saved = first.save();

    let second = persisting("x");
    second.load(saved).unwrap();
    assert_eq!(second.get::<Words>(&"a".into()), ["x"]);
    assert_eq!(second.computed::<Words>().len(), 1, "the memo of b is never restored");
}

/// How many words the sources `0` to `key` hold, read through `Words`.
struct Length;
impl Query for Length {
    const NAME: &'static str = "length";
    type Key = u32;
    type Value = usize;
    fn compute(cx: &Cx<'_>, key: &u32) -> usize {
        (0..*key).map(|i| cx.get::<Words>(&i.to_string()).len()).sum()
    }
}

#[test]
fn a_key_reached_by_its_saved_position_and_by_its_value_has_one_slot() {
    let sources = |db: &Db| (0..20).for_each(|i: u32| db.set::<Source>(&i.to_string(), "a b".into()));
    let first = persisting("");
    first.persist::<Length>();
    sources(&first);
    assert_eq!(first.get::<Length>(&20), 40);
    let saved = first.save();

    // Verifying the length reaches each word memo by its saved position, its key unread; a
    // source changed then makes the length ask for the words by their keys.
    let second = persisting("");
    second.persist::<Length>();
    second.load(saved).unwrap();
    sources(&second);
    assert_eq!(second.get::<Length>(&20), 40);
    second.set::<Source>(&"7".into(), "a b c".into());
    assert_eq!(second.get::<Length>(&20), 41);
    assert_eq!(Runs::<Words>::of(&second), 1, "only the words of the source changed are computed");
    let keys: Vec<String> = second.computed::<Words>().into_iter().map(|(key, _)| key).collect();
    let distinct: std::collections::HashSet<&String> = keys.iter().collect();
    assert_eq!((keys.len(), distinct.len()), (20, 20), "one slot per key");
}

#[test]
fn a_key_threads_add_at_once_is_saved_once() {
    const THREADS: usize = 8;
    let db = persisting("one two");
    let keys: Vec<String> = (0..400).map(|n| n.to_string()).collect();
    let start = std::sync::Barrier::new(THREADS);
    std::thread::scope(|s| {
        for _ in 0..THREADS {
            s.spawn(|| {
                start.wait();
                for key in &keys {
                    db.get::<Words>(key);
                }
            });
        }
    });
    let census = Census::of(&db.save()).unwrap();
    let words = census.queries.iter().find(|q| q.name == Words::NAME).unwrap();
    assert_eq!((words.entries, words.memos), (keys.len(), keys.len()));
}
