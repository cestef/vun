//! Compile-time evaluation through the whole front end, over the toolchain's `lang` and `std`.

mod common;

use common::Fixture;
use overt_diag::errors;

#[test]
fn each_instantiation_of_a_type_function_is_its_own_type() {
    let fixture = Fixture::new("comptime-instances");
    let file = fixture.root.write(
        "pairs.ov",
        "\
Pair := ($T: type) -> type { struct { a: T, b: T } };
same := (p: Pair(u8)) -> Pair(u8) { p };
other := (p: Pair(u8)) -> Pair(u16) { p };
narrow := (p: Pair(u8)) -> u8 { p.a };
wide := (p: Pair(u16)) -> u8 { p.b };
",
    );
    assert_eq!(
        fixture.reported(&file),
        [
            (errors::TYPES_MISMATCH, "expected `Pair(u16)`, found `Pair(Int(0..=255))`".to_owned()),
            (errors::INT_DOES_NOT_FIT, "value does not fit the type `u8`".to_owned()),
        ]
    );
}
