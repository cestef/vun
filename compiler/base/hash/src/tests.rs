use std::path::Path;

use super::{Algorithm, Change, Hash, Snapshot, base32};

#[test]
fn text_form_names_the_algorithm_and_round_trips() {
    let h = Hash::of(b"overt");
    let text = h.to_string();
    assert!(text.starts_with("blake3:") && text.len() == "blake3:".len() + 52, "{text}");
    assert_eq!(Hash::parse(&text), Some(h));
    assert_eq!(h.algorithm(), Algorithm::Blake3);
    assert_eq!(Hash::parse(&text.replace(':', "-")), None);
    assert_eq!(Hash::parse(&text.replace("blake3", "md5")), None);
    assert_eq!(Hash::parse("blake3:zz"), None);
}

#[test]
fn base32_matches_rfc_4648_and_has_one_spelling() {
    // RFC 4648, section 10, in lowercase without padding.
    assert_eq!(base32::encode(b"foobar"), "mzxw6ytboi");
    assert_eq!(base32::decode::<6>("mzxw6ytboi"), Some(*b"foobar"));
    assert_eq!(base32::decode::<6>("mzxw6ytboj"), None);
}

#[test]
fn trees_depend_on_contents_and_names_only() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let whole = Hash::tree(&src, &["lib.rs".into(), "tree.rs".into()]);
    assert_eq!(whole, Hash::tree(&src, &["tree.rs".into(), "lib.rs".into(), "absent".into()]));
    assert_ne!(whole, Hash::tree(&src, &["lib.rs".into()]));
    assert_ne!(Hash::tree(&src, &["lib.rs".into()]), Hash::of(&std::fs::read(src.join("lib.rs")).unwrap()));
}

#[test]
fn snapshots_round_trip_and_name_changed_files() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let all = Snapshot::of(&src, &["lib.rs".into(), "tree.rs".into()]);
    assert_eq!(Snapshot::parse(&all.to_string()), Some(all.clone()));
    assert_eq!(all.root(), Hash::tree(&src, &["tree.rs".into(), "lib.rs".into()]));
    let fewer = Snapshot::of(&src, &["lib.rs".into(), "tests.rs".into()]);
    let changes = all.changes(&fewer);
    assert_eq!(changes, [("tests.rs".to_owned(), Change::Added), ("tree.rs".to_owned(), Change::Removed)]);
    assert_eq!(Snapshot::parse("blake3:zz lib.rs"), None);
}

#[test]
fn snapshots_refuse_paths_leaving_their_tree() {
    let h = Hash::of(b"x");
    for path in ["../x", "a/../../x", "/etc/x", "a//b", ".", "a\\b"] {
        assert_eq!(Snapshot::parse(&format!("{h} {path}")), None, "{path}");
    }
    assert!(Snapshot::parse(&format!("{h} a/b.ov")).is_some());
}
