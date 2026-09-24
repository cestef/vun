//! Loops: heads, items through `lang.ops.Iterate` (the table `iteration`), and `break` values.

use crate::common::Fixture;
use vun_diag::errors;

#[test]
fn loop_items_and_values_are_typed() {
    let fixture = Fixture::new("loops-typed");
    let file = fixture.root.write(
        "loops.vun",
        "\
count := (n: u8) { for i in 0..n { x: bool = i; } };
each := (xs: &[u16]) { for x in xs { y: bool = x; } };
array := (xs: [2]u8) { for x in xs { y: bool = x; } };
value := () -> bool { for { break 3; } };
labeled := () -> u8 { 'l: for { for { break 'l 300; } } };
test := (o: ?u8) { mut c := o; for .some(v) = c { w: bool = v; c = .none; } };
",
    );
    let found = |ty: &str| (errors::TYPES_MISMATCH, format!("expected `bool`, found `{ty}`"));
    assert_eq!(
        fixture.reported(&file),
        [
            found("Int(0..=254)"),
            found("&Int(0..=65535)"),
            found("Int(0..=255)"),
            found("Int(3..=3)"),
            (errors::INT_DOES_NOT_FIT, "value does not fit the type `u8`".to_owned()),
            found("Int(0..=255)"),
        ]
    );
}
