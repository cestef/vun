//! `.replace`: the root manifest substitutes a package for an identity anywhere in the graph, and
//! the lockfile records it.

use crate::common::Fixture;
use vun_build::manifest::Identity;

const HAL: Identity = Identity { id: 7, major: 1 };

#[test]
fn replacements_apply_everywhere_and_are_locked() {
    let fx = Fixture::new("replace");
    let hal = Fixture::manifest("hal", HAL.id, "1.0.0");
    fx.write("hal", &hal, "a := 1;\n");
    let (url, hash) = fx.publish("hal", &hal);
    fx.write("hal-fork", &Fixture::manifest("hal", HAL.id, "1.0.1"), "a := 1;\nb := 2;\n");
    let mut app = Fixture::manifest("app", 1, "0.1.0");
    app.prelude.push("hal".into());
    app.deps.push(Fixture::stored("hal", "1.0.0", &url, hash));
    app.replace.push(Fixture::replace("hal", HAL, "../hal-fork"));
    let dir = fx.write("app", &app, "x := b;\n");

    let mut session = fx.session();
    let project = session.package_at(&dir).unwrap();
    session.fetch(project);
    let lock = session.write_lock(&dir, project).unwrap();
    assert!(!session.failed(), "{:?}", session.diag.sorted(session.origins));
    let replaced = r#".{ .name = "hal", .id = .{ .id = 7, .major = 1 }, .path = "../hal-fork" },"#;
    assert!(lock.to_string().contains(replaced), "{lock}");
    // `b` exists only in the fork.
    assert_eq!(fx.check(&Fixture::file(&dir, &app)), []);
}
