use std::path::PathBuf;

use super::{Captured, Edit, Mode, Modes, Refusal, Relative, Scratch, Tree, WORK};

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
fn installs_keep_what_is_already_there() {
    let dir = Scratch::new("install");
    let target = dir.join("store/entry");
    dir.write("first/a.vun", "first");
    dir.write("second/a.vun", "second");
    target.install(&dir.join("first")).unwrap();
    target.install(&dir.join("second")).unwrap();
    assert_eq!(std::fs::read_to_string(target.join("a.vun")).unwrap(), "first");
    assert!(!dir.join("first").exists() && !dir.join("second").exists());
}

#[test]
fn threads_putting_one_file_each_write_their_own_part() {
    let dir = Scratch::new("put");
    let path = dir.join("f");
    let contents: Vec<_> = (0..8u8).map(|i| vec![i; 1 << 16]).collect();
    std::thread::scope(|s| {
        for bytes in &contents {
            let path = &path;
            s.spawn(move || path.put(bytes).unwrap());
        }
    });
    assert!(contents.contains(&std::fs::read(&path).unwrap()));
    assert_eq!(dir.path().entries().unwrap(), [path]);
}

#[cfg(unix)]
#[test]
fn walks_refuse_links() {
    let dir = Scratch::new("links");
    dir.write("a/b.vun", "");
    std::os::unix::fs::symlink("/etc", dir.join("a/link")).unwrap();
    let e = dir.join("a").files().unwrap_err();
    assert_eq!(e.get_ref().and_then(|e| e.downcast_ref()), Some(&Refusal::Link(dir.join("a/link"))));
}

/// What a walk skips is never read: a link below it is no refusal.
#[cfg(unix)]
#[test]
fn walks_skip_what_they_are_told_to_without_reading_it() {
    let dir = Scratch::new("skip");
    dir.write("a/b.vun", "");
    dir.write("a/tool/c.js", "");
    std::os::unix::fs::symlink("/etc", dir.join("a/tool/link")).unwrap();
    let tool = dir.join("a/tool");
    let files = dir.join("a").files_except(&|p| p == tool).unwrap();
    assert_eq!(names(&files, dir.path()), ["a/b.vun"]);
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

#[test]
#[cfg(target_os = "linux")]
fn a_mount_point_is_read_with_its_escapes() {
    assert_eq!(
        crate::Mounts::unescaped(br"/run/media/a\040b\134c"),
        std::path::PathBuf::from("/run/media/a b\\c")
    );
    assert_eq!(
        crate::Mounts::unescaped(br"/a\04"),
        std::path::PathBuf::from("/a\\04"),
        "a short escape is kept"
    );
}

#[test]
fn a_directory_is_made_with_its_parents_and_may_be_there() {
    let dir = Scratch::new("make");
    let deep = dir.path().join("a").join("b");
    deep.make().unwrap();
    deep.make().unwrap();
    assert!(deep.is_dir());
}

#[cfg(unix)]
#[test]
fn a_command_ends_with_its_process_not_with_a_daemon_holding_its_streams() {
    let started = std::time::Instant::now();
    // The background command keeps the streams as long as it sleeps.
    let mut command = std::process::Command::new("sh");
    command.args(["-c", "echo out; echo err >&2; sleep 5 & exit 3"]);
    let output = Captured::run(&mut command).unwrap();
    assert_eq!((output.stdout, output.stderr), (b"out\n".to_vec(), b"err\n".to_vec()));
    assert_eq!(output.status.code(), Some(3));
    assert!(started.elapsed() < std::time::Duration::from_secs(3));
}
