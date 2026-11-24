//! Reducing a failing file (spec 13, section 10): the least text that reports the error kept.

use vun_diag::{DiagId, errors};

use crate::support::Fixture;

const FILE: &str = "\
helper := (a: u8) -> u8 { a };

main := () -> i32 {
    x := helper(1);
    y: bool = x;
    0
};

other := 5;
";

#[test]
fn a_file_is_reduced_to_the_least_that_reports_the_error_kept() {
    let fixture = Fixture::new("reduce");
    let path = fixture.root.write("p.vun", FILE);
    let mut session = fixture.session();
    let file = session.load(&path).unwrap();
    let reduced = session.reduce(file, Some(errors::TYPES_MISMATCH)).unwrap();
    assert_eq!(reduced.text, "main := () { 0 };\n");
    assert_eq!(fixture.check(&fixture.root.write("reduced.vun", &reduced.text)), [errors::TYPES_MISMATCH]);
}

#[test]
fn a_file_without_the_error_kept_has_nothing_to_reduce() {
    let fixture = Fixture::new("reduce-nothing");
    let path = fixture.root.write("p.vun", "main := () {};\n");
    let mut session = fixture.session();
    let file = session.load(&path).unwrap();
    assert!(session.reduce(file, Some(DiagId::compiler("types.mismatch"))).is_err());
    let ids: Vec<DiagId> = session.diag.sorted(session.origins).into_iter().map(|d| d.id).collect();
    assert_eq!(ids, [errors::TOOL_NOTHING_TO_REDUCE]);
}
