//! Memos shared between databases: those reading shared inputs alone, saved apart and loaded by a
//! database that holds none of its own.

use super::Runs;
use crate::{Census, Cx, Db, Query, Reuse};

static LENGTHS: Runs = Runs::new();
static JOINED: Runs = Runs::new();

/// A file's text by path, an input.
struct Text;
impl Query for Text {
    const NAME: &'static str = "text";
    const INPUT: bool = true;
    type Key = String;
    type Value = String;
    fn compute(_: &Cx<'_>, _: &String) -> String {
        String::new()
    }
}

/// A file's length, read from its text.
struct Length;
impl Query for Length {
    const NAME: &'static str = "length";
    type Key = String;
    type Value = usize;
    fn compute(cx: &Cx<'_>, key: &String) -> usize {
        LENGTHS.tick();
        cx.get::<Text>(key).len()
    }
}

/// Two files' lengths added, reading both.
struct Joined;
impl Query for Joined {
    const NAME: &'static str = "joined";
    type Key = (String, String);
    type Value = usize;
    fn compute(cx: &Cx<'_>, (a, b): &(String, String)) -> usize {
        JOINED.tick();
        cx.get::<Length>(a) + cx.get::<Length>(b)
    }
}

/// A database with `lib` shared and `main` its own, as a library and a project.
fn project(main: &str) -> Db {
    let db = Db::reusing(Reuse::Trust);
    db.trace::<Text>();
    db.persist::<Length>();
    db.persist::<Joined>();
    db.share::<Text>(&"lib".into(), "library".into());
    db.set::<Text>(&"main".into(), main.into());
    db
}

#[test]
fn memos_reading_shared_inputs_alone_are_saved_apart_and_reused() {
    let first = project("main one");
    let both = ("lib".to_owned(), "main".to_owned());
    assert_eq!(first.get::<Joined>(&both), 15);
    let shared = first.save_shared().expect("a shared memo was computed");
    // The library's length, and its text's trace; neither the project's nor what reads it.
    let memos = Census::of(&shared).unwrap().queries.iter().map(|q| q.memos).collect::<Vec<_>>();
    assert_eq!(memos, [0, 1, 0]);

    // Another project starts from them: the library is not computed again.
    let second = project("another");
    second.load(shared.clone()).unwrap();
    let runs = LENGTHS.count();
    assert_eq!(second.get::<Joined>(&both), 14);
    assert_eq!(LENGTHS.count(), runs + 1, "the project's length alone");
    assert_eq!(second.save_shared(), None, "no shared memo was computed");

    // A shared memo whose input changed is recomputed, never trusted.
    let edited = project("another");
    edited.share::<Text>(&"lib".into(), "lib".into());
    edited.load(shared).unwrap();
    assert_eq!(edited.get::<Joined>(&both), 10);
    assert!(edited.save_shared().is_some());
    assert!(JOINED.count() >= 3);
}
