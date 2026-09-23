use std::path::PathBuf;

use super::{Edit, Scratch, Tree};

fn names(paths: &[PathBuf], root: &std::path::Path) -> Vec<String> {
    paths.iter().map(|p| p.strip_prefix(root).unwrap().to_string_lossy().into_owned()).collect()
}

#[test]
fn walks_are_sorted_and_a_file_is_its_own_walk() {
    let dir = Scratch::new("walk");
    dir.write("b.ov", "");
    dir.write("a/z.ov", "");
    dir.write("a/y.ov", "");
    assert_eq!(names(&dir.path().files().unwrap(), dir.path()), ["a/y.ov", "a/z.ov", "b.ov"]);
    assert_eq!(names(&dir.path().entries().unwrap(), dir.path()), ["a", "b.ov"]);
    assert_eq!(dir.join("b.ov").files().unwrap(), [dir.join("b.ov")]);
    assert!(dir.join("absent").files().is_err());
}

#[test]
fn edits_create_parents_and_replace_whole_directories() {
    let dir = Scratch::new("edit");
    let target = dir.join("store/entry");
    dir.write("store/entry/old.ov", "old");
    dir.write("staging/new.ov", "new");
    target.replace_with(&dir.join("staging")).unwrap();
    assert_eq!(names(&target.files().unwrap(), &target), ["new.ov"]);
    assert!(!dir.join("staging").exists());
    target.fresh().unwrap();
    assert!(target.entries().unwrap().is_empty());
    target.remove().unwrap();
    target.remove().unwrap();
}

#[test]
fn scratch_directories_are_removed_when_dropped() {
    let path = Scratch::new("drop").path().to_path_buf();
    assert!(!path.exists());
}
