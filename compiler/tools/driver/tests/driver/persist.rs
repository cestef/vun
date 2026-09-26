//! Queries saved by one session and loaded by the next (spec 13, section 2): a saved memo is
//! verified before it is used, so an edit costs recomputation, never a wrong result.

use std::path::Path;

use vun_diag::DiagId;
use vun_driver::Persistence;

use crate::common::Fixture;

/// The diagnostics of checking `file` in a fresh session that loads the queries saved beside it
/// and saves its own there.
fn persisted(fixture: &Fixture, file: &Path) -> Vec<DiagId> {
    let mut session = fixture.session();
    session.persistence = Persistence::On;
    let file = session.load(file).unwrap();
    session.check(file);
    session.save();
    session.diag.sorted(session.origins).into_iter().map(|d| d.id.clone()).collect()
}

#[test]
fn a_declaration_an_edit_moved_is_not_typed_at_its_old_place() {
    let fixture = Fixture::new("persist");
    let before = "\
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit { .code(0) };
";
    // The local declaration takes the place `main` had among the file's declarations.
    let after = "\
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit { x: u8 = 1; .code(x) };
";
    let path = fixture.root.write("app.vun", before);
    assert_eq!(persisted(&fixture, &path), []);
    fixture.root.write("app.vun", after);
    assert_eq!(persisted(&fixture, &path), []);
}
