use std::path::Path;

use vun_fs::{Mode, Modes, Refusal, Scratch};

use super::{Algorithm, Change, Hash, Snapshot, base32};

#[test]
fn text_form_names_the_algorithm_and_round_trips() {
    let h = Hash::of(b"vun");
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
    let whole = Hash::tree(&src, &["lib.rs".into(), "tree.rs".into()]).unwrap();
    assert_eq!(whole, Hash::tree(&src, &["tree.rs".into(), "lib.rs".into(), "absent".into()]).unwrap());
    assert_ne!(whole, Hash::tree(&src, &["lib.rs".into()]).unwrap());
    assert_ne!(
        Hash::tree(&src, &["lib.rs".into()]).unwrap(),
        Hash::of(&std::fs::read(src.join("lib.rs")).unwrap())
    );
}

#[test]
fn snapshots_round_trip_and_name_changed_files() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let all = Snapshot::of(&src, &["lib.rs".into(), "tree.rs".into()]).unwrap();
    assert_eq!(Snapshot::parse(&all.to_string()), Some(all.clone()));
    assert_eq!(all.root(), Hash::tree(&src, &["tree.rs".into(), "lib.rs".into()]).unwrap());
    let fewer = Snapshot::of(&src, &["lib.rs".into(), "tests.rs".into()]).unwrap();
    let changes = all.changes(&fewer);
    assert_eq!(changes, [("tests.rs".to_owned(), Change::Added), ("tree.rs".to_owned(), Change::Removed)]);
    assert_eq!(Snapshot::parse("blake3:zz lib.rs"), None);
}

#[test]
fn snapshots_refuse_paths_leaving_their_tree() {
    let h = Hash::of(b"x");
    for path in ["../x", "a/../../x", "/etc/x", "a//b", ".", "a\\b"] {
        assert_eq!(Snapshot::parse(&format!("{h} - {path}")), None, "{path}");
    }
    assert!(Snapshot::parse(&format!("{h} - a/b.vun")).is_some());
    assert_eq!(Snapshot::parse(&format!("{h} a/b.vun")), None, "the mode is required");
    assert_eq!(Snapshot::parse(&format!("{h} rw a/b.vun")), None);
}

#[test]
fn the_executable_bit_is_part_of_the_tree() {
    let dir = Scratch::new("mode");
    let script = dir.write("configure", "#!/bin/sh\n");
    let include = ["configure".to_owned()];
    let regular = Snapshot::of(dir.path(), &include).unwrap();
    script.set_mode(Mode::Executable).unwrap();
    let executable = Snapshot::of(dir.path(), &include).unwrap();
    if script.mode() == Mode::Executable {
        assert_ne!(regular.root(), executable.root());
        assert!(executable.to_string().contains(" x configure"), "{executable}");
        assert_eq!(Snapshot::parse(&executable.to_string()), Some(executable.clone()));
        assert_eq!(regular.changes(&executable), [("configure".to_owned(), Change::Mode)]);
    }
}

#[test]
fn a_value_hashes_as_its_bytes_however_they_are_written() {
    use std::hash::Hasher as _;
    let bytes: Vec<u8> = (0..20_000u32).map(|n| (n % 251) as u8).collect();
    for step in [1, 7, 1000, 5000, 20_000] {
        let mut hasher = super::Hasher::default();
        for part in bytes.chunks(step) {
            hasher.write(part);
        }
        assert_eq!(hasher.finish_hash(), Hash::of(&bytes), "{step}");
    }
}

#[test]
fn snapshots_refuse_entries_leaving_their_tree_and_links() {
    let dir = Scratch::new("refuse");
    dir.write("a/b.vun", "");
    for entry in ["../a", "/etc", "a/../a"] {
        let e = Snapshot::of(dir.path(), &[entry.to_owned()]).unwrap_err();
        assert_eq!(e.get_ref().and_then(|e| e.downcast_ref()), Some(&Refusal::Outside(entry.to_owned())));
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("/etc", dir.join("a/link")).unwrap();
        let e = Snapshot::of(dir.path(), &["a".to_owned()]).unwrap_err();
        assert_eq!(e.get_ref().and_then(|e| e.downcast_ref()), Some(&Refusal::Link(dir.join("a/link"))));
    }
}

#[test]
fn taken_snapshots_keep_what_they_hashed() {
    let dir = Scratch::new("take");
    dir.write("a/b.vun", "b");
    dir.write("c.vun", "c");
    let taken = Snapshot::take(dir.path(), &["a".to_owned(), "c.vun".to_owned()]).unwrap();
    let files: Vec<_> =
        taken.files().map(|(path, entry, bytes)| (path.join("/"), entry.hash, bytes)).collect();
    assert_eq!(
        files,
        [("a/b.vun".to_owned(), Hash::of(b"b"), &b"b"[..]), ("c.vun".to_owned(), Hash::of(b"c"), &b"c"[..])]
    );
}

#[test]
fn a_file_is_no_directory_in_a_snapshot() {
    let h = Hash::of(b"x");
    assert_eq!(Snapshot::parse(&format!("{h} - a\n{h} - a/b")), None);
    assert!(Snapshot::parse(&format!("{h} - a\n{h} - ab/c")).is_some());
}

#[test]
fn a_reader_is_hashed_as_the_bytes_it_gives() {
    let bytes: Vec<u8> = (0..200_000u32).map(|n| n.to_le_bytes()[0]).collect();
    assert_eq!(Hash::of_reader(bytes.as_slice()).unwrap(), Hash::of(&bytes));
}

#[test]
fn every_field_of_a_snapshot_entry_changes_the_tree_hash() {
    let tree = |files: &[(&str, &[u8], Mode)]| {
        let dir = Scratch::new("fields");
        for &(name, bytes, mode) in files {
            dir.write(name, bytes).set_mode(mode).unwrap();
        }
        let include: Vec<String> = files.iter().map(|&(name, ..)| name.to_owned()).collect();
        Snapshot::of(dir.path(), &include).unwrap().root()
    };
    let base = tree(&[("a", b"x", Mode::Regular)]);
    assert_ne!(base, tree(&[("a", b"y", Mode::Regular)]), "contents");
    assert_ne!(base, tree(&[("b", b"x", Mode::Regular)]), "name");
    assert_ne!(base, tree(&[("a", b"x", Mode::Executable)]), "mode");
}
