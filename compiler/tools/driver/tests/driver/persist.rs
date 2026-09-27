//! Queries saved by one session and loaded by the next (spec 13, section 2): a saved memo is
//! verified before it is used, so an edit costs recomputation, never a wrong result.

use std::path::Path;

use vun_diag::{DiagId, errors};
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
main := (sys: &mut std.os.System) -> std.os.Exit { .ok };
";
    // The local declaration takes the place `main` had among the file's declarations.
    let after = "\
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit { x: std.os.Status = 1; .code(x) };
";
    let path = fixture.root.write("app.vun", before);
    assert_eq!(persisted(&fixture, &path), []);
    fixture.root.write("app.vun", after);
    assert_eq!(persisted(&fixture, &path), []);
}

#[test]
fn a_saved_report_is_shown_where_an_edit_above_moved_it() {
    use vun_origin::Resolve;
    use vun_span::SourceMap;
    let fixture = Fixture::new("persist-moved");
    let body = "\
unread := () -> u8 {
    mut x: u8;
    x
};
";
    let path = fixture.root.write("app.vun", body);
    // The line of the read in each text, as the session shows it.
    let shown = |text: &str| {
        fixture.root.write("app.vun", text);
        let mut session = fixture.session();
        session.persistence = Persistence::On;
        let file = session.load(&path).unwrap();
        session.check(file);
        session.save();
        let reports = session.diag.sorted(&session);
        let read = reports.iter().find(|d| d.id == errors::PLACE_READ_UNINIT).expect("the read is reported");
        let origin = read.primary.as_ref().unwrap().origin;
        session.sources.locate(session.source_span(origin)).at.line
    };
    assert_eq!(shown(body), 3);
    assert_eq!(shown(&format!("\n\n\n{body}")), 6);
}
