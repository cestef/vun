//! Saved memos of any bytes, or of a saved file with bytes changed, load and are read without
//! panicking: what cannot be decoded is recomputed, never trusted.

#![no_main]

use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use vun_query::{Census, Cx, Db, Query};

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
        cx.get::<Source>(key).split_whitespace().map(str::to_owned).collect()
    }
}

const FILES: [(&str, &str); 2] = [("a", "one two three"), ("b", "four")];

/// A database with the persisted queries registered and the sources of [`FILES`] set.
fn persisting() -> Db {
    let db = Db::new();
    db.trace::<Source>();
    db.persist::<Words>();
    for (path, text) in FILES {
        db.set::<Source>(&path.into(), text.into());
    }
    db
}

/// The memos of every file, as saved once per process.
fn saved() -> &'static [u8] {
    static SAVED: OnceLock<Vec<u8>> = OnceLock::new();
    SAVED.get_or_init(|| {
        let db = persisting();
        for (path, _) in FILES {
            db.get::<Words>(&path.into());
        }
        db.save()
    })
}

/// Loads `bytes`, reads every memo and saves again; the file saved loads too.
fn exercise(bytes: Vec<u8>) {
    let db = persisting();
    if db.load(bytes).is_err() {
        return;
    }
    for (path, _) in FILES {
        db.get::<Words>(&path.into());
    }
    let again = persisting();
    again.load(db.save()).expect("a saved file loads");
}

fuzz_target!(|data: &[u8]| {
    let _ = Census::of(data);
    exercise(data.to_vec());
    // The input also names a place in a real file and bytes flipped there.
    let [low, high, flips @ ..] = data else { return };
    let mut bytes = saved().to_vec();
    let at = usize::from(u16::from_le_bytes([*low, *high])) % bytes.len();
    for (byte, flip) in bytes[at..].iter_mut().zip(flips) {
        *byte ^= flip;
    }
    let _ = Census::of(&bytes);
    exercise(bytes);
});
