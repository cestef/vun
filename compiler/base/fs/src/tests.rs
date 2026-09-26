use std::path::PathBuf;

use super::{Edit, Mode, Modes, Relative, Scratch, Tree, WORK};

fn names(paths: &[PathBuf], root: &std::path::Path) -> Vec<String> {
    paths.iter().map(|p| p.strip_prefix(root).unwrap().to_string_lossy().into_owned()).collect()
}

#[test]
fn walks_are_sorted_and_a_file_is_its_own_walk() {
    let dir = Scratch::new("walk");
    dir.write("b.vun", "");
    dir.write("a/z.vun", "");
    dir.write("a/y.vun", "");
    assert_eq!(names(&dir.path().files().unwrap(), dir.path()), ["a/y.vun", "a/z.vun", "b.vun"]);
    assert_eq!(names(&dir.path().entries().unwrap(), dir.path()), ["a", "b.vun"]);
    assert_eq!(dir.join("b.vun").files().unwrap(), [dir.join("b.vun")]);
    assert!(dir.join("absent").files().is_err());
}

#[test]
fn edits_create_parents_and_replace_whole_directories() {
    let dir = Scratch::new("edit");
    let target = dir.join("store/entry");
    dir.write("store/entry/old.vun", "old");
    dir.write("staging/new.vun", "new");
    target.replace_with(&dir.join("staging")).unwrap();
    assert_eq!(names(&target.files().unwrap(), &target), ["new.vun"]);
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

#[test]
fn relative_paths_climb_to_the_common_directory() {
    let dir = Scratch::new("relative");
    dir.write("app/build.vun", "");
    dir.write("libs/util/build.vun", "");
    let relative = dir.join("libs/util").relative_to(&dir.join("app")).unwrap();
    assert_eq!(relative, std::path::Path::new("../libs/util"));
    assert_eq!(dir.join("app").relative_to(&dir.join("app")).unwrap(), std::path::Path::new("."));
    assert!(dir.join("absent").relative_to(dir.path()).is_none());
}

#[cfg(unix)]
#[test]
fn modes_are_set_and_read_back() {
    let dir = Scratch::new("mode");
    let path = dir.write("configure", "#!/bin/sh\n");
    assert_eq!(path.mode(), Mode::Regular);
    path.set_mode(Mode::Executable).unwrap();
    assert_eq!(path.mode(), Mode::Executable);
    path.set_mode(Mode::Regular).unwrap();
    assert_eq!(path.mode(), Mode::Regular);
}

#[test]
fn walks_leave_out_the_toolchain_directory() {
    let dir = Scratch::new("work");
    dir.path().join("a.vun").put("").unwrap();
    dir.path().join(WORK).join("queries").join("x").put("").unwrap();
    assert_eq!(dir.path().files().unwrap(), [dir.path().join("a.vun")]);
}

#[test]
fn a_file_put_replaces_the_old_whole_and_leaves_no_part() {
    let dir = Scratch::new("put");
    let path = dir.path().join("a").join("b");
    path.put("old").unwrap();
    path.put("new").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"new");
    assert_eq!(dir.path().files().unwrap(), [path]);
}
