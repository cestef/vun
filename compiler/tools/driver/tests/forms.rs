//! Borrows, `unsafe` blocks, attributed type literals, struct patterns, and reports at the
//! branch that does not fit.

mod common;

use common::Fixture;
use overt_diag::errors;

#[test]
fn borrows_attributes_and_struct_patterns_are_typed() {
    let fixture = Fixture::new("forms-typed");
    let file = fixture.root.write(
        "forms.ov",
        "\
Linear := #[kind.linear] struct { channel: u8 };
Point := struct { x: u8, y: u16 };
channel := (t: &take Linear) -> bool { t.channel };
borrow := (x: u8) -> bool { &x };
region := (x: u8) -> bool { unsafe { x } };
fields := (p: Point) -> bool { .{ .x = a, .y = b } := p; b };
arm := (c: bool, x: u8, y: u16) -> u8 { if c { x } else { y } };
",
    );
    let found = |ty: &str| (errors::TYPES_MISMATCH, format!("expected `bool`, found `{ty}`"));
    assert_eq!(
        fixture.reported(&file),
        [
            found("Int(0..=255)"),
            found("&Int(0..=255)"),
            found("Int(0..=255)"),
            found("Int(0..=65535)"),
            (errors::INT_DOES_NOT_FIT, "value does not fit the type `u8`".to_owned()),
        ]
    );
}
