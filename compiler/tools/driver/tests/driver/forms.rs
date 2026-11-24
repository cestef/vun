//! Borrows, `unsafe` blocks, attributed type literals, struct patterns, and reports at the
//! branch that does not fit.

use crate::support::Fixture;
use vun_diag::errors;

#[test]
fn borrows_attributes_and_struct_patterns_are_typed() {
    let fixture = Fixture::new("forms-typed");
    let file = fixture.root.write(
        "forms.vun",
        "\
Linear := #[kind.linear] struct { channel: u8 };
Point := struct { x: u8, y: u16 };
channel := (t: &take Linear) -> bool { t.channel };
borrow := (x: u8) -> bool { &x };
region := (x: u8) -> bool { unsafe { x } };
fields := (p: Point) -> bool { .{ .x = _, .y = b } := p; b };
arm := (c: bool, x: u8, y: u16) -> u8 { if c { x } else { y } };
",
    );
    let found = |ty: &str| (errors::TYPES_MISMATCH, format!("expected `bool`, found `{ty}`"));
    assert_eq!(
        fixture.reported(&file),
        [
            found("u8"),
            found("&u8"),
            found("u8"),
            found("u16"),
            (errors::INT_DOES_NOT_FIT, "value does not fit the type `u8`".to_owned()),
        ]
    );
}

#[test]
fn temporaries_are_borrowed_as_expected_and_empty_literals_are_unit() {
    let fixture = Fixture::new("forms-temporaries");
    let file = fixture.root.write(
        "temporaries.vun",
        "\
read := (x: &u8) -> u8 { x.* };
seven := () -> u8 { read(&7) };
narrow: Int(7..=7) = 7;
place := () -> u8 { read(&narrow) };
empty := () { e := .{}; _: () = e; _: u8 = e; };
",
    );
    // `&7` borrows a `u8` holding 7; `narrow` keeps its own type, which `&u8` does not accept.
    let found = fixture.reported(&file);
    let ids: Vec<_> = found.iter().map(|(id, _)| *id).collect();
    assert_eq!(ids, [errors::TYPES_MISMATCH, errors::TYPES_MISMATCH], "{found:?}");
    assert!(found[0].1.contains("&static Int(7..=7)"), "{found:?}");
    assert!(found[1].1.ends_with("found `()`"), "{found:?}");
}

#[test]
fn a_member_a_dollar_argument_gave_a_type_without_is_reported_at_the_call() {
    let fixture = Fixture::new("forms-no-member");
    let file = fixture.root.write(
        "member.vun",
        "\
first := ($F: type, f: F) -> F.Params { f };
bad := () { first((u8) -> u8, add_one); };
add_one := (x: u8) -> u8 { x };
",
    );
    let found = fixture.reported(&file);
    let ids: Vec<_> = found.iter().map(|(id, _)| *id).collect();
    assert_eq!(ids, [errors::COMPTIME_TRAP], "{found:?}");
    assert!(found[0].1.contains("has no member `Params`"), "{found:?}");
}
