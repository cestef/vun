//! `overt pkg fetch` end to end: a package archive served by a `file://` mirror is stored,
//! verified, used through the prelude, and a stored file changed afterwards is named.

use overt_build::store::Store;
use overt_diag::errors;
use overt_driver::Session;
use overt_driver::packages::Fetched;
use overt_fs::Scratch;
use overt_pkg::Archive;

const HAL: &str = r#"package := .{
    .name = "hal", .id = 2, .version = "1.0.0", .edition = 2027,
    .include = .{ "build.ov", "hal.ov" },
};
"#;

#[test]
fn fetched_packages_are_stored_verified_and_checked() {
    let root = Scratch::new("fetch");
    let hal = root.join("hal-src");
    root.write("hal-src/build.ov", HAL);
    root.write("hal-src/hal.ov", "gpio := 1;\n");
    let (bytes, snapshot) = Archive::pack(&hal, &["build.ov".into(), "hal.ov".into()]).unwrap();
    let archive = root.write("hal.ovpkg", bytes);
    let hash = snapshot.root();

    let app = root.join("app");
    let manifest = format!(
        "package := .{{ .name = \"app\", .id = 1, .version = \"0.1.0\", .edition = 2027,\n\
         .include = .{{ \"build.ov\", \"main.ov\" }}, .prelude = .{{ \"std.prelude\", \"hal\" }},\n\
         .deps = .{{ .hal = .{{ .version = \"1.0.0\", .hash = \"{hash}\", .urls = .{{ \"file://{}\" }} }} }} }};\n",
        archive.display()
    );
    root.write("app/build.ov", &manifest);
    root.write("app/main.ov", "x := gpio;\n");

    let mut session = Session::new();
    session.store = Some(Store::in_project(&app));
    let project = session.package_at(&app).unwrap();
    let fetched: Vec<_> = session.fetch(project).into_iter().map(|(_, h, f)| (h, f)).collect();
    assert!(matches!(fetched.as_slice(), [(h, Fetched::Stored { .. })] if *h == hash), "{fetched:?}");
    assert!(!session.failed());
    assert_eq!(session.fetch(project).last().map(|(_, _, f)| f.clone()), Some(Fetched::Present));

    // The stored package resolves `gpio` through the prelude.
    let mut check = Session::new();
    let file = check.load(&app.join("main.ov")).unwrap();
    check.check(file);
    assert!(!check.failed(), "{:?}", check.diag.sorted().iter().map(|d| d.id.clone()).collect::<Vec<_>>());

    // A stored file changed afterwards is named.
    root.write(Store::in_project(&app).package(hash).join("hal.ov"), "gpio := 2;\n");
    let mut check = Session::new();
    let file = check.load(&app.join("main.ov")).unwrap();
    check.check(file);
    let mismatch = check.diag.sorted().into_iter().find(|d| d.id == errors::PACKAGE_HASH_MISMATCH).unwrap();
    let notes: Vec<_> = mismatch.children.iter().map(|c| c.msg.to_string()).collect();
    assert!(notes.iter().any(|n| n.contains("hal.ov") && n.contains("modified")), "{notes:?}");
}

#[test]
fn failing_mirrors_are_reported_with_their_causes() {
    let root = Scratch::new("fail");
    let hash = overt_hash::Hash::of(b"none");
    let manifest = format!(
        "package := .{{ .name = \"app\", .id = 1, .version = \"0.1.0\", .edition = 2027,\n\
         .include = .{{ \"build.ov\" }},\n\
         .deps = .{{ .hal = .{{ .version = \"1.0.0\", .hash = \"{hash}\", .urls = .{{ \"file://{}/absent.ovpkg\" }} }} }} }};\n",
        root.path().display()
    );
    root.write("build.ov", &manifest);
    let mut session = Session::new();
    session.store = Some(Store::in_project(root.path()));
    let project = session.package_at(root.path()).unwrap();
    assert!(matches!(session.fetch(project).as_slice(), [(_, _, Fetched::Failed)]));
    let d = session.diag.sorted().into_iter().find(|d| d.id == errors::PACKAGE_FETCH_FAILED).unwrap();
    let kinds: Vec<_> = d.children.iter().map(|c| c.kind.name()).collect();
    assert_eq!(kinds, ["note", "caused by", "caused by"], "{:?}", d.children);
}
