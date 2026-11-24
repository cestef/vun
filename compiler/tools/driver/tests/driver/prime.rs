//! A toolchain primed with its libraries' memos (spec 13, section 2): a program checked from an
//! empty store starts with the libraries computed, wherever the libraries are.

use std::path::{Path, PathBuf};

use vun_driver::Persistence;
use vun_driver::prime::Prime;
use vun_fs::Tree;
use vun_harness::repo_root;
use vun_query::Event;

use crate::support::Fixture;

const APP: &str = "\
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit { .ok };
";

/// How many bodies a session typed that no memo held, checking `app` with the toolchain's memos
/// at `shipped`, the libraries at `library`, and no store.
fn typed_anew(fixture: &Fixture, app: &Path, library: &Path, shipped: &Path) -> u64 {
    let mut session = fixture.session();
    session.library = library.to_path_buf();
    session.shared = Some(fixture.root.join("unshared"));
    session.shipped = Some(shipped.to_path_buf());
    session.persistence = Persistence::On;
    let file = session.load(app).unwrap();
    session.check(file);
    session.db.tally().count("typed", Event::Absent)
}

/// The libraries copied to `to`, which is another place.
fn moved(fixture: &Fixture, to: &str) -> PathBuf {
    let lib = repo_root().join("lib");
    for file in lib.files().unwrap() {
        let within = file.strip_prefix(&lib).unwrap();
        fixture.root.write(Path::new(to).join(within), std::fs::read(&file).unwrap());
    }
    fixture.root.join(to)
}

#[test]
fn a_program_starts_from_the_memos_of_a_primed_toolchain_whatever_the_place_of_its_libraries() {
    let fixture = Fixture::new("prime");
    let lib = repo_root().join("lib");
    let memos = fixture.root.join("memos");
    let primed = Prime { library: lib.clone(), memos: memos.clone() }.run().unwrap();
    assert_eq!(primed.diag.error_count(), 0, "the libraries check clean");
    assert!(primed.files > 0 && primed.bytes > 0);
    let app = fixture.root.write("app.vun", APP);
    // Without the memos, checking the program types thousands of bodies of the libraries.
    let unprimed = typed_anew(&fixture, &app, &lib, &fixture.root.join("nothing"));
    assert!(unprimed > 500, "{unprimed}");
    assert!(typed_anew(&fixture, &app, &lib, &memos) < unprimed / 10);
    let elsewhere = moved(&fixture, "elsewhere");
    assert!(typed_anew(&fixture, &app, &elsewhere, &memos) < unprimed / 10);
}
