//! `overt pkg add`: a path and an archive are declared in the manifest, the archive with the hash
//! it had when first fetched, and the project then resolves through them.

use crate::common::Fixture;
use overt_build::layout::MANIFEST;
use overt_build::manifest::{DepKind, Source, Version};
use overt_diag::errors;
use overt_driver::Session;
use overt_driver::packages::{Addition, Unstated};
use overt_pkg::{Format, Scheme};

#[test]
fn added_dependencies_are_declared_and_resolve() {
    let fx = Fixture::new("add");
    let hal = Fixture::manifest("hal", 2, "1.4.0");
    fx.write("hal", &hal, "gpio := 1;\n");
    let (url, hash) = fx.publish("hal", &hal);
    let util = fx.write("util", &Fixture::manifest("util", 3, "0.1.0"), "x := 1;\n");
    let mut app = Fixture::manifest("app", 1, "0.1.0");
    app.prelude.push(hal.name.clone());
    let dir = fx.write("app", &app, "y := gpio + util.x;\n");

    let mut session = fx.session();
    let add = |session: &mut Session, source: &str| {
        session.add(&dir, &Addition { source, name: None, version: None })
    };
    let added = add(&mut session, &url).unwrap().unwrap();
    assert!(matches!(added.source, Source::Archive { hash: h, .. } if h == hash));
    let util = util.to_str().unwrap();
    assert!(add(&mut session, util).unwrap().is_some());
    assert!(add(&mut session, util).unwrap().is_none());
    let ids: Vec<_> = session.diag.sorted().into_iter().map(|d| d.id.clone()).collect();
    assert_eq!(ids, [errors::PACKAGE_DUPLICATE_DEP]);

    let text = std::fs::read_to_string(dir.join(MANIFEST)).unwrap();
    assert!(text.contains(".util = .{ .path = \"../util\" }"), "{text}");
    assert_eq!(fx.check(&Fixture::file(&dir, &app)), [], "{text}");
}

#[test]
fn foreign_archives_need_a_name_and_a_version() {
    let fx = Fixture::new("foreign");
    let dir = fx.write("app", &Fixture::manifest("app", 1, "0.1.0"), "");
    fx.root.write("sdk/x.c", "int x;\n");
    let (bytes, _) = Format::PUBLISHED.pack(&fx.root.join("sdk"), &["x.c".into()], "sdk").unwrap();
    let archive = fx.root.write(format!("sdk{}", Format::PUBLISHED.suffix()), bytes);
    let url = Scheme::file_url(&archive);
    let mut session = fx.session();
    let unnamed = Addition { source: &url, name: None, version: None };
    assert_eq!(session.add(&dir, &unnamed).unwrap_err(), Unstated::Name);
    let unversioned = Addition { source: &url, name: Some("sdk"), version: None };
    assert_eq!(session.add(&dir, &unversioned).unwrap_err(), Unstated::Version);
    let named = Addition { source: &url, name: Some("sdk"), version: Version::parse("2.0.0") };
    assert_eq!(session.add(&dir, &named).unwrap().unwrap().kind, DepKind::Foreign);
    assert!(std::fs::read_to_string(dir.join(MANIFEST)).unwrap().contains(".kind = .foreign"));
}
