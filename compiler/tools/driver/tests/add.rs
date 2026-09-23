//! `overt pkg add`: a path and an archive are declared in the manifest, the archive with the hash
//! it had when first fetched, and the project then resolves through them.

use overt_build::store::Store;
use overt_diag::errors;
use overt_driver::Session;
use overt_driver::packages::{Addition, Unstated};
use overt_fs::Scratch;
use overt_pkg::Archive;

fn manifest(name: &str, extra: &str) -> String {
    format!(
        "package := .{{\n    .name = \"{name}\", .id = 1, .version = \"1.4.0\", .edition = 2027,\n    \
         .include = .{{ \"build.ov\", \"{name}.ov\" }},{extra}\n}};\n"
    )
}

#[test]
fn added_dependencies_are_declared_and_resolve() {
    let root = Scratch::new("add");
    root.write("hal/build.ov", manifest("hal", ""));
    root.write("hal/hal.ov", "gpio := 1;\n");
    root.write("util/build.ov", manifest("util", ""));
    root.write("util/util.ov", "x := 1;\n");
    root.write("app/build.ov", manifest("app", "\n    .prelude = .{ \"std.prelude\", \"hal\" },"));
    root.write("app/app.ov", "y := gpio + util.x;\n");
    let (bytes, snapshot) = Archive::pack(&root.join("hal"), &["build.ov".into(), "hal.ov".into()]).unwrap();
    let archive = root.write("hal.ovpkg", bytes);

    let app = root.join("app");
    let mut session = Session::new();
    session.store = Some(Store::in_project(&app));
    let url = format!("file://{}", archive.display());
    let add = |session: &mut Session, source: &str| {
        let util = root.join("util");
        let source = if source == "util" { util.to_str().unwrap() } else { source };
        session.add(&app, &Addition { source, name: None, version: None })
    };
    let hal = add(&mut session, &url).unwrap().unwrap();
    assert!(
        matches!(hal.source, overt_build::manifest::Source::Archive { hash, .. } if hash == snapshot.root())
    );
    assert!(add(&mut session, "util").unwrap().is_some());
    assert!(add(&mut session, "util").unwrap().is_none());
    let ids: Vec<_> = session.diag.sorted().into_iter().map(|d| d.id.clone()).collect();
    assert_eq!(ids, [errors::PACKAGE_DUPLICATE_DEP]);

    let text = std::fs::read_to_string(app.join("build.ov")).unwrap();
    assert!(text.contains(".util = .{ .path = \"../util\" }"), "{text}");
    let mut check = Session::new();
    let file = check.load(&app.join("app.ov")).unwrap();
    check.check(file);
    assert!(
        !check.failed(),
        "{text}\n{:?}",
        check.diag.sorted().iter().map(|d| d.id.clone()).collect::<Vec<_>>()
    );
}

#[test]
fn foreign_archives_need_a_name_and_a_version() {
    let root = Scratch::new("foreign");
    root.write("app/build.ov", manifest("app", ""));
    root.write("sdk/x.c", "int x;\n");
    let (bytes, _) = Archive::pack(&root.join("sdk"), &["x.c".into()]).unwrap();
    let archive = root.write("sdk.ovpkg", bytes);
    let url = format!("file://{}", archive.display());
    let app = root.join("app");
    let mut session = Session::new();
    session.store = Some(Store::in_project(&app));
    let unnamed = Addition { source: &url, name: None, version: None };
    assert_eq!(session.add(&app, &unnamed).unwrap_err(), Unstated::Name);
    let unversioned = Addition { source: &url, name: Some("sdk"), version: None };
    assert_eq!(session.add(&app, &unversioned).unwrap_err(), Unstated::Version);
    let version = overt_build::manifest::Version::parse("2.0.0");
    let named = Addition { source: &url, name: Some("sdk"), version };
    let dep = session.add(&app, &named).unwrap().unwrap();
    assert_eq!(dep.kind, overt_build::manifest::DepKind::Foreign);
    assert!(std::fs::read_to_string(app.join("build.ov")).unwrap().contains(".kind = .foreign"));
}
