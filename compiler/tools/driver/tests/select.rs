//! Minimal version selection: the largest version any package requires is the one every dependent
//! uses, the lockfile records it, and a lockfile that does not is reported.

mod common;

use common::Fixture;
use overt_build::lock::{self, Lock, Locked};
use overt_build::manifest::Identity;
use overt_diag::errors;
use overt_hash::Hash;

const HAL_ID: u64 = 7;

#[test]
fn the_largest_required_version_is_selected_everywhere() {
    let fx = Fixture::new("select");
    let (old_hal, new_hal) =
        (Fixture::manifest("hal", HAL_ID, "1.0.0"), Fixture::manifest("hal", HAL_ID, "1.2.0"));
    fx.write("hal-1.0", &old_hal, "a := 1;\n");
    fx.write("hal-1.2", &new_hal, "a := 1;\nb := 2;\n");
    let (old_url, old) = fx.publish("hal-1.0", &old_hal);
    let (new_url, new) = fx.publish("hal-1.2", &new_hal);
    let mut drv = Fixture::manifest("drv", 8, "1.0.0");
    drv.deps.push(Fixture::stored("hal", "1.2.0", &new_url, new));
    fx.write("drv", &drv, "");
    let mut app = Fixture::manifest("app", 1, "0.1.0");
    app.prelude.push("hal".into());
    app.deps.extend([Fixture::stored("hal", "1.0.0", &old_url, old), Fixture::local("drv", "../drv")]);
    let dir = fx.write("app", &app, "x := b;\n");

    let mut session = fx.session();
    let project = session.package_at(&dir).unwrap();
    session.fetch(project);
    let selection = session.write_lock(&dir, project);
    assert!(!session.failed(), "{:?}", session.diag.sorted());
    assert_eq!(selection.entries().len(), 2, "every required version is recorded: {selection}");
    let selected = selection.selected(Identity { id: HAL_ID, major: 1 }).unwrap();
    assert_eq!(
        (selected.version, selected.hash, selected.via.as_deref()),
        (new_hal.version, new, Some("drv"))
    );

    // The lockfile roots both in the store, labeled; an entry nothing records is collected.
    let rooted: Vec<_> = fx.store().list().unwrap().into_iter().filter(|l| l.rooted).collect();
    let labels: Vec<_> = rooted.iter().filter_map(|l| l.label.as_deref()).collect();
    assert_eq!(labels.len(), 2);
    assert!(labels.contains(&"hal 1.0.0") && labels.contains(&"hal 1.2.0"), "{labels:?}");
    fx.root.write(fx.store().package(Hash::of(b"stray")).join("x.ov"), "");
    let collected = fx.store().collect().unwrap();
    assert_eq!((collected.removed.len(), collected.kept), (1, 2));

    // The app declares hal 1.0.0 but sees 1.2.0, which has `b`.
    let app_file = Fixture::file(&dir, &app);
    assert_eq!(fx.check(&app_file), []);

    // A lockfile recording something else is stale.
    let stale = Lock::new(vec![Locked { hash: old, ..selected.clone() }]);
    fx.root.write(dir.join(lock::FILE), stale.to_string());
    assert!(fx.check(&app_file).contains(&errors::PACKAGE_STALE_LOCK));
}
