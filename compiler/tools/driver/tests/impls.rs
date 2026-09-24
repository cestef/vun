//! Impls and witnesses (spec 02, section 3), over the toolchain's `lang` and `std`.

mod common;

use common::Fixture;
use overt_diag::errors;

#[test]
fn methods_come_from_witnesses_then_interface_defaults() {
    let fixture = Fixture::new("impls-methods");
    let file = fixture.root.write(
        "shapes.ov",
        "\
Shape := interface(($Self: type) -> type { struct {
    area: (self: &Self) -> u64 = _;
    sides := (self: &Self) -> u8 { 4 };
}});
Square := struct { side: u16 };
#[impl(Shape(Square))]
SquareShape := struct { area := (self: &Square) -> u16 { self.side }; };
area := (s: Square) -> bool { s.area() };
sides := (s: Square) -> bool { s.sides() };
",
    );
    assert_eq!(
        fixture.reported(&file),
        [
            (errors::TYPES_MISMATCH, "expected `bool`, found `Int(0..=65535)`".to_owned()),
            (errors::TYPES_MISMATCH, "expected `bool`, found `Int(0..=255)`".to_owned()),
        ]
    );
}

#[test]
fn an_impl_for_a_type_constructor_covers_every_type_it_makes() {
    let fixture = Fixture::new("impls-constructors");
    let file = fixture.root.write(
        "pairs.ov",
        "\
Pair := ($T: type) -> type { struct { a: T, b: T } };
First := interface(($Self: type) -> type { struct {
    count := (p: Self) -> u8 { 2 };
}});
#[impl(First(Pair))]
PairFirst := ($T: type) -> type { struct { first := (p: Pair(T)) -> T { p.a }; } };
first := (p: Pair(u16)) -> bool { p.first() };
count := (p: Pair(bool)) -> bool { p.count() };
#[impl(First(Pair(u8)))]
Again := struct {};
",
    );
    assert_eq!(
        fixture.reported(&file),
        [
            (errors::TYPES_MISMATCH, "expected `bool`, found `Int(0..=65535)`".to_owned()),
            (errors::TYPES_MISMATCH, "expected `bool`, found `Int(0..=255)`".to_owned()),
            (errors::IMPL_DUPLICATE, "a second impl of `First(Pair(Int(0..=255)))`".to_owned()),
        ]
    );
}
