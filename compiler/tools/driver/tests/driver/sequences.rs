//! Arrays, slices and ranges: `lang.mem.Array`, `lang.mem.Slice` and `lang.Bounds`, with their
//! methods from `std` impls (decision 78).

use crate::common::Fixture;
use overt_diag::errors;

#[test]
fn lengths_bounds_and_parts_are_typed() {
    let fixture = Fixture::new("sequences-typed");
    let file = fixture.root.write(
        "seq.ov",
        "\
last := (xs: &[u32]) -> u32 { xs[xs.len() - 1] };
count := (xs: &[u32]) -> bool { xs.len() };
four := (a: [4]u8) -> bool { a.len() };
tail := (xs: &[u8], k: u8) -> &[u8] { xs[k..] };
part := (xs: &[u8]) -> u8 { xs[1..3] };
upto := (n: u8) -> bool { 0..n };
Digit := Int(0..10);
digit: Digit = 9;
",
    );
    let found = |ty: &str| (errors::TYPES_MISMATCH, format!("expected `bool`, found `{ty}`"));
    assert_eq!(
        fixture.reported(&file),
        [
            found("Int(0..=18446744073709551615)"),
            found("Int(0..=18446744073709551615)"),
            (errors::TYPES_MISMATCH, "expected `u8`, found `&[Int(0..=255)]`".to_owned()),
            found("Bounds(Int(0..=254))"),
        ]
    );
}
