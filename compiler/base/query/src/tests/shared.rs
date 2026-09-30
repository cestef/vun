//! Memos shared between databases: those reading shared inputs alone, saved apart and loaded by a
//! database that holds none of its own, which saves its own memos layered over them.

use super::Runs;
use crate::{Census, Cx, Db, Layering, Query, Reuse, Unloaded};

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

#[test]
fn memos_saved_over_shared_ones_hold_the_project_s_alone() {
    let both = ("lib".to_owned(), "main".to_owned());
    let first = project("main one");
    first.get::<Joined>(&both);
    let shared = first.save_shared().expect("a shared memo was computed");
    let id = Layering::of(&shared).unwrap().id;
    assert!(id.is_some(), "a shared file is named by its body");

    let second = project("main two");
    second.load_over(shared.clone(), None).unwrap();
    assert_eq!(second.get::<Joined>(&both), 15);
    let own = second.save();
    assert_eq!(Layering::of(&own).unwrap(), Layering { id: None, base: id });
    // The project's length and what joins them; the library's stays in the shared file.
    let memos = Census::of(&own).unwrap().queries.iter().map(|q| q.memos).collect::<Vec<_>>();
    assert_eq!(memos, [0, 1, 1]);
    assert!(matches!(project("main two").load(own.clone()), Err(Unloaded::Base)));
    assert!(matches!(project("main two").load_over(own.clone(), None), Err(Unloaded::Base)));

    // Both loaded again, nothing is computed.
    let third = project("main two");
    third.load_over(shared.clone(), Some(own)).unwrap();
    let (lengths, joined) = (LENGTHS.count(), JOINED.count());
    assert_eq!(third.get::<Joined>(&both), 15);
    assert_eq!((LENGTHS.count(), JOINED.count()), (lengths, joined));

    // A shared memo computed again is saved over the one it replaces, and holds next time.
    let edited = || {
        let db = project("main two");
        db.share::<Text>(&"lib".into(), "lib".into());
        db
    };
    let fourth = edited();
    fourth.load_over(shared.clone(), None).unwrap();
    assert_eq!(fourth.get::<Joined>(&both), 11);
    let own = fourth.save();
    let memos = Census::of(&own).unwrap().queries.iter().map(|q| q.memos).collect::<Vec<_>>();
    assert_eq!(memos, [0, 2, 1], "the library's length replaced");
    let fifth = edited();
    fifth.load_over(shared.clone(), Some(own)).unwrap();
    let lengths = LENGTHS.count();
    assert_eq!(fifth.get::<Joined>(&both), 11);
    assert_eq!(LENGTHS.count(), lengths, "the replaced memo holds");

    // A file extending one layered over the shared file stays layered over it.
    fifth.get::<Length>(&"other".into());
    let grown = fifth.save();
    assert_eq!(Layering::of(&grown).unwrap().base, id);
    let sixth = edited();
    sixth.load_over(shared, Some(grown)).unwrap();
    let lengths = LENGTHS.count();
    assert_eq!(sixth.get::<Joined>(&both), 11);
    sixth.get::<Length>(&"other".into());
    assert_eq!(LENGTHS.count(), lengths, "every memo of both layers holds");
}

#[test]
fn a_database_that_loaded_nothing_saves_over_the_shared_memos_it_saved() {
    let both = ("lib".to_owned(), "main".to_owned());
    let first = project("main one");
    first.get::<Joined>(&both);
    let shared = first.save_shared().expect("a shared memo was computed");
    assert_eq!(first.save_shared(), None, "saved already");
    let own = first.save();
    assert_eq!(Layering::of(&own).unwrap().base, Layering::of(&shared).unwrap().id);
    let memos = Census::of(&own).unwrap().queries.iter().map(|q| q.memos).collect::<Vec<_>>();
    assert_eq!(memos, [0, 1, 1], "the project's alone");

    let second = project("main one");
    second.load_over(shared, Some(own)).unwrap();
    let (lengths, joined) = (LENGTHS.count(), JOINED.count());
    assert_eq!(second.get::<Joined>(&both), 15);
    assert_eq!((LENGTHS.count(), JOINED.count()), (lengths, joined));
}

#[test]
fn shared_memos_a_shared_file_loaded_lacks_are_saved_with_the_own() {
    let first = project("main one");
    first.get::<Length>(&"lib".into());
    let shared = first.save_shared().expect("a shared memo was computed");
    let with_extra = || {
        let db = project("main one");
        db.share::<Text>(&"extra".into(), "more".into());
        db
    };

    let second = with_extra();
    second.load_over(shared.clone(), None).unwrap();
    assert_eq!(second.get::<Length>(&"extra".into()), 4);
    assert_eq!(second.save_shared(), None, "the shared file loaded is written once");
    let own = second.save();
    assert_eq!(Layering::of(&own).unwrap().base, Layering::of(&shared).unwrap().id);

    let third = with_extra();
    third.load_over(shared, Some(own)).unwrap();
    let lengths = LENGTHS.count();
    assert_eq!(third.get::<Length>(&"extra".into()), 4);
    assert_eq!(LENGTHS.count(), lengths, "the shared memo saved with the own holds");
}
