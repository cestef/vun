//! `overt pkg fetch` end to end: a package archive served by a `file://` mirror is stored,
//! verified, used through the prelude, and a stored file changed afterwards is named.

use crate::common::Fixture;
use overt_diag::errors;
use overt_driver::packages::Fetched;
use overt_pkg::{Format, Scheme};

#[test]
fn fetched_packages_are_stored_verified_and_checked() {
    let fx = Fixture::new("fetch");
    let hal = Fixture::manifest("hal", 2, "1.0.0");
    fx.write("hal-src", &hal, "gpio := 1;\n");
    let (url, hash) = fx.publish("hal-src", &hal);
    let mut app = Fixture::manifest("app", 1, "0.1.0");
    app.prelude.push("hal".into());
    app.deps.push(Fixture::stored("hal", "1.0.0", &url, hash));
    let dir = fx.write("app", &app, "x := gpio;\n");

    let mut session = fx.session();
    let project = session.package_at(&dir).unwrap();
    let fetched: Vec<_> = session.fetch(project).into_iter().map(|(_, h, f)| (h, f)).collect();
    assert!(matches!(fetched.as_slice(), [(h, Fetched::Stored { .. })] if *h == hash), "{fetched:?}");
    assert!(!session.failed());
    assert_eq!(session.fetch(project).last().map(|(_, _, f)| f.clone()), Some(Fetched::Present));

    // The stored package resolves `gpio` through the prelude.
    let file = Fixture::file(&dir, &app);
    assert_eq!(fx.check(&file), []);

    // A stored file changed afterwards is named.
    let stored = fx.store().package(hash);
    fx.root.write(Fixture::file(&stored, &hal), "gpio := 2;\n");
    let mut check = fx.session();
    let loaded = check.load(&file).unwrap();
    check.check(loaded);
    let mismatch = check.diag.sorted().into_iter().find(|d| d.id == errors::PACKAGE_HASH_MISMATCH).unwrap();
    let notes: Vec<_> = mismatch.children.iter().map(|c| c.msg.to_string()).collect();
    assert!(notes.iter().any(|n| n.contains("hal.ov") && n.contains("modified")), "{notes:?}");
}

#[test]
fn failing_mirrors_are_reported_with_their_causes() {
    let fx = Fixture::new("fail");
    let url = Scheme::file_url(&fx.root.join(format!("absent{}", Format::PUBLISHED.suffix())));
    let mut app = Fixture::manifest("app", 1, "0.1.0");
    app.deps.push(Fixture::stored("hal", "1.0.0", &url, overt_hash::Hash::of(b"none")));
    let dir = fx.write("app", &app, "");
    let mut session = fx.session();
    let project = session.package_at(&dir).unwrap();
    assert!(matches!(session.fetch(project).as_slice(), [(_, _, Fetched::Failed)]));
    let d = session.diag.sorted().into_iter().find(|d| d.id == errors::PACKAGE_FETCH_FAILED).unwrap();
    let kinds: Vec<_> = d.children.iter().map(|c| c.kind.name()).collect();
    assert_eq!(kinds, ["note", "caused by", "caused by"], "{:?}", d.children);
}
