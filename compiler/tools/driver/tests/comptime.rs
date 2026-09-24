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

#[test]
fn operators_are_typed_by_their_lang_ops_interfaces() {
    let fixture = Fixture::new("comptime-operators");
    let file = fixture.root.write(
        "ops.ov",
        "\
less := (a: u8, b: u8) -> bool { a < b };
differ := (a: u8) -> bool { a != 3 };
bits := (a: u8, b: u8) -> u8 { a & b };
equal := (a: u8, b: u8) -> u8 { a == b };
",
    );
    assert_eq!(fixture.reported(&file), [(errors::TYPES_MISMATCH, "expected `u8`, found `bool`".to_owned())]);
}

#[test]
fn branches_run_at_compile_time_and_join_their_types() {
    let fixture = Fixture::new("comptime-branches");
    let file = fixture.root.write(
        "branches.ov",
        "\
max := ($a: Int(0..=100), $b: Int(0..=100)) -> Int(0..=100) { if a > b { a } else { b } };
pick := ($n: u8) -> u8 { if n == 1 and !(n > 5) or n == 7 { 10 } else { 20 } };
Low := Int(0..=max(3, 70));
Picked := Int(0..=pick(1) + pick(2));
low: Low = 71;
picked: Picked = 30;
join := (x: u8, y: u16) -> u8 { if x < 3 { x } else { y } };
head := (x: u8) -> u8 { if x { 1 } else { 2 } };
",
    );
    assert_eq!(
        fixture.reported(&file),
        [
            (errors::INT_DOES_NOT_FIT, "value does not fit the type `Low`".to_owned()),
            (errors::INT_DOES_NOT_FIT, "value does not fit the type `u8`".to_owned()),
            (errors::TYPES_MISMATCH, "expected `bool`, found `Int(0..=255)`".to_owned()),
        ]
    );
}
