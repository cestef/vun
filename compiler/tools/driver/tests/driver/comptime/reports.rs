//! Library diagnostics (spec 10, section 3): compile-time code reports with the compiler's model,
//! pointing inside the literal a caller wrote through `Code` arguments and `$if` tests.

use crate::common::Fixture;
use vun_diag::errors;

#[test]
fn a_format_string_problem_is_reported_at_the_bytes_of_the_literal_it_covers() {
    let fixture = Fixture::new("comptime-reports");
    let text = "\
format := ($spec: Code(&[u8])) {
    $if .err(bad) = std.fmt.parse($spec) {
        $std.diag.error(.{ .at = .some(spec.within(bad.at)), .msg = bad.reason.text() });
    }
};
calls := () {
    format(\"ok {} here\");
    format(\"a\\n{y} b\");
    format(\"left } over\");
    format(\"open {\");
};
";
    let file = fixture.root.write("reports.vun", text);
    let mut session = fixture.session();
    let file = session.load(&file).unwrap();
    session.check(file);
    let reported = session.diag.sorted(session.origins);
    assert!(reported.iter().all(|d| d.id == errors::COMPTIME_REPORTED), "{reported:#?}");
    // Each report points at the bytes the problem covers, an escape before them included.
    let placed: Vec<&str> = reported
        .iter()
        .filter_map(|d| d.primary.as_ref())
        .map(|p| session.origins.source_span(p.origin).range.of(text))
        .collect();
    assert_eq!(placed, ["{y}", "}", "{"]);
}
