//! Queries saved by one session and loaded by the next (spec 13, section 2): a saved memo is
//! verified before it is used, so an edit costs recomputation, never a wrong result.

use std::path::Path;

use vun_diag::{DiagId, errors};
use vun_driver::{Persistence, Session};

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

/// A package `app` whose root file calls `app.util.code` and whose `app/util.vun` holds `util`,
/// built through lowering in a fresh session that loads and saves its queries.
fn build(fixture: &Fixture, util: &str) -> Session {
    let mut app = Fixture::manifest("app", 1, "0.1.0");
    app.include.push(app.name.clone());
    let dir = fixture.write(
        "",
        &app,
        "\
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit { app.util.code(sys) };
",
    );
    fixture.root.write(dir.join(&app.name).join("util.vun"), util);
    let mut session = fixture.session();
    session.persistence = Persistence::On;
    let file = session.load(&Fixture::file(&dir, &app)).unwrap();
    session.check(file);
    session.program(file);
    session.save();
    session
}

/// The diagnostics of [`build`], every memo the session reuses checked against computing it
/// again (`Reuse::Check`).
fn built(fixture: &Fixture, util: &str) -> Vec<DiagId> {
    let session = build(fixture, util);
    session.diag.sorted(session.origins).into_iter().map(|d| d.id.clone()).collect()
}

#[test]
fn declarations_an_edit_moved_in_another_file_are_reused_soundly() {
    let fixture = Fixture::new("persist-moved-elsewhere");
    let code = "code := (sys: &mut std.os.System) -> std.os.Exit { .ok };\n";
    assert_eq!(built(&fixture, code), []);
    // Declarations inserted above move `code`, whose origin, anchored to its name, stays.
    assert_eq!(built(&fixture, &format!("first := 1;\nsecond := () -> u8 {{ 2 }};\n{code}")), []);
    assert_eq!(built(&fixture, code), []);
}

#[test]
fn a_signature_an_edit_changed_in_another_file_is_checked_again() {
    let fixture = Fixture::new("persist-signature-elsewhere");
    assert_eq!(built(&fixture, "code := (sys: &mut std.os.System) -> std.os.Exit { .ok };\n"), []);
    let changed = "code := (sys: &mut std.os.System) -> u8 { 0 };\n";
    assert_eq!(built(&fixture, changed), [errors::TYPES_MISMATCH]);
}

#[test]
fn an_unchanged_program_built_again_computes_nothing_it_saves() {
    let fixture = Fixture::new("persist-unchanged");
    let code = "code := (sys: &mut std.os.System) -> std.os.Exit { .ok };\n";
    drop(build(&fixture, code));
    // A memo reading a query that is not saved is never saved either, and so computed again.
    assert!(!build(&fixture, code).db.changed());
}
