use std::path::Path;

use vun_fs::Scratch;
use vun_hash::Hash;

use super::Store;
use crate::lock::{self, Lock, Locked};
use crate::manifest::Version;

#[test]
fn paths_nest_by_algorithm_then_digest_prefix() {
    let hash = Hash::of(b"package");
    let digest = hash.digest();
    let store = Store::at(Path::new("s"));
    let entry = Path::new("s/blake3").join(&digest[..2]).join(&digest[2..]);
    assert_eq!(store.entry(hash), entry);
    assert_eq!(store.package(hash), entry.join("package"));
    assert_eq!(store.snapshot(hash), entry.join("snapshot"));
    assert_eq!(store.label(hash), entry.join("label"));
}

#[test]
fn collection_keeps_what_registered_lockfiles_record() {
    let dir = Scratch::new("collect");
    let store = Store::at(&dir.join("store"));
    let [kept, dropped] = [Hash::of(b"kept"), Hash::of(b"dropped")];
    for hash in [kept, dropped] {
        dir.write(store.package(hash).join("a.vun"), "a := 1;\n");
    }
    dir.write(store.label(kept), "hal 1.0.0\n");
    let locked = Locked {
        name: "hal".into(),
        version: Version::parse("1.0.0").unwrap(),
        identity: None,
        hash: kept,
        via: None,
    };
    dir.write(dir.join("app").join(lock::FILE), Lock::new(vec![locked]).to_string());
    store.register(&dir.join("app")).unwrap();

    let listed = store.list().unwrap();
    let kept_entry = listed.iter().find(|l| l.hash == kept).unwrap();
    assert_eq!((kept_entry.label.as_deref(), kept_entry.rooted), (Some("hal 1.0.0"), true));
    let collected = store.collect().unwrap();
    assert_eq!((collected.removed.len(), collected.removed[0].hash, collected.kept), (1, dropped, 1));
    assert!(store.entry(kept).exists() && !store.entry(dropped).exists());

    // A project whose lockfile is gone roots nothing.
    std::fs::remove_file(dir.join("app").join(lock::FILE)).unwrap();
    assert_eq!(store.collect().unwrap().removed[0].hash, kept);
    assert!(store.list().unwrap().is_empty());
}
