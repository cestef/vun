//! Generic code is checked per instantiation and reported at the definition (spec 02, section 3).

use crate::common::Fixture;
use overt_diag::errors;

#[test]
fn generic_bodies_are_typed_per_instantiation() {
    let fixture = Fixture::new("generics-instances");
    let file = fixture.root.write(
        "generic.ov",
        "\
narrow := ($T: type, x: T) -> u8 { x };
fine := () -> u8 { narrow(u8, 3) };
wide := () -> u8 { narrow(u16, 300) };
Pair := ($T: type) -> type { struct {
    a: T,
    small := (p: Pair(T)) -> u8 { p.a };
} };
ok := (p: Pair(u8)) -> u8 { p.small() };
bad := (p: Pair(u32)) -> u8 { p.small() };
",
    );
    assert_eq!(
        fixture.reported(&file),
        [
            (errors::INT_DOES_NOT_FIT, "value does not fit the type `u8`".to_owned()),
            (errors::INT_DOES_NOT_FIT, "value does not fit the type `u8`".to_owned()),
        ]
    );
}
