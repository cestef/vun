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

#[test]
fn a_report_carries_its_help_and_the_fixes_that_replace_what_it_points_at() {
    let fixture = Fixture::new("comptime-reports-fixes");
    let text = "\
format := ($spec: Code(&[u8])) {
    $if .err(bad) = std.fmt.parse($spec) {
        $std.diag.error(.{
            .at = .some(spec.within(bad.at)),
            .msg = bad.reason.text(),
            .help = .some(\"write the argument after the string\"),
            .fixes = &[.{ .msg = \"write `{}`\", .safety = .safe, .text = \"{}\" }],
        });
    }
};
calls := () { format(\"a {y} b\"); };
";
    let file = fixture.root.write("fixes.vun", text);
    let mut session = fixture.session();
    let file = session.load(&file).unwrap();
    session.check(file);
    let reported = session.diag.sorted(session.origins);
    let [d] = reported.as_slice() else { panic!("{reported:#?}") };
    assert_eq!(
        d.children.iter().map(|c| c.msg.to_string()).collect::<Vec<_>>(),
        ["write the argument after the string"]
    );
    let [fix] = d.fixes.as_slice() else { panic!("{d:#?}") };
    let [edit] = fix.edits.as_slice() else { panic!("{fix:#?}") };
    assert_eq!(
        (edit.span, edit.text.as_str()),
        (session.origins.source_span(d.primary.as_ref().unwrap().origin), "{}")
    );
}

#[test]
fn a_dyn_of_an_interface_with_requirements_it_cannot_call_is_reported() {
    let fixture = Fixture::new("comptime-reports-dyn");
    let text = "\
ByValue := interface(($Self: type) -> type { struct {
    consume: (self: Self) -> u64 = _;
} });
Named := interface(($Self: type) -> type { struct {
    Name: type = _;
    size: (self: &Self) -> u64 = _;
} });
Thing := struct { n: u64 };
impl ByValue for Thing { consume := (self) -> u64 { self.n }; }
impl Named for Thing {
    Name: type = u8;
    size := (self) -> u64 { self.n };
}
by_value := (t: &Thing) {
    _ = unsafe std.Dyn(ByValue).of(Thing, lang.mem.pointer_to(Thing, t));
};
named := (t: &Thing) {
    _ = unsafe std.Dyn(Named).of(Thing, lang.mem.pointer_to(Thing, t));
};
";
    let file = fixture.root.write("dyn.vun", text);
    let mut session = fixture.session();
    let file = session.load(&file).unwrap();
    session.check(file);
    let reported = session.diag.sorted(session.origins);
    assert!(!reported.is_empty(), "a requirement a table cannot hold is reported");
    assert!(reported.iter().all(|d| d.id == errors::COMPTIME_REPORTED), "{reported:#?}");
}
