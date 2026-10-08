//! Impls and witnesses (spec 02, section 3), over the toolchain's `lang` and `std`.

use crate::common::Fixture;
use vun_diag::errors;
use vun_sema::typed::Typed;
use vun_sema::witness::{Resolved, Source};
use vun_sema_ty::call::Instantiation;

#[test]
fn methods_come_from_witnesses_then_interface_defaults() {
    let fixture = Fixture::new("impls-methods");
    let file = fixture.root.write(
        "shapes.vun",
        "\
Shape := interface(($Self: type) -> type { struct {
    area: (self: &Self) -> u64 = _;
    sides := (self: &Self) -> u8 { 4 };
}});
Square := struct { side: u16 };
#[impl(Shape(Square))]
square_shape := struct { area := (self: &Square) -> u64 { self.side }; };
area := (s: Square) -> bool { s.area() };
sides := (s: Square) -> bool { s.sides() };
",
    );
    assert_eq!(
        fixture.reported(&file),
        [
            (errors::TYPES_MISMATCH, "expected `bool`, found `u64`".to_owned()),
            (errors::TYPES_MISMATCH, "expected `bool`, found `u8`".to_owned()),
        ]
    );
}

#[test]
fn an_impl_for_a_type_constructor_covers_every_type_it_makes() {
    let fixture = Fixture::new("impls-constructors");
    let file = fixture.root.write(
        "pairs.vun",
        "\
Pair := ($T: type) -> type { struct { a: T, b: T } };
First := interface(($Self: type) -> type { struct {
    count := (p: Self) -> u8 { 2 };
}});
#[impl(First(Pair))]
pair_first := ($T: type) -> type { struct { first := (p: Pair(T)) -> T { p.a }; } };
first := (p: Pair(u16)) -> bool { p.first() };
count := (p: Pair(bool)) -> bool { p.count() };
#[impl(First(Pair(u8)))]
Again := struct {};
",
    );
    assert_eq!(
        fixture.reported(&file),
        [
            (errors::TYPES_MISMATCH, "expected `bool`, found `u16`".to_owned()),
            (errors::TYPES_MISMATCH, "expected `bool`, found `u8`".to_owned()),
            (errors::IMPL_DUPLICATE, "a second impl of `First(Pair(u8))`".to_owned()),
        ]
    );
}

#[test]
fn integer_methods_come_from_the_std_impl_for_lang_int() {
    let fixture = Fixture::new("impls-integers");
    let file = fixture.root.write(
        "narrow.vun",
        "\
narrow := (x: u32) -> (u8, u16, u8, ?u8) { (x.to(u8), x.wrap(u16), x.sat(u8), x.try_to(u8)) };
wrong := (x: u32) -> u8 { x.wrap(u16) };
half := ($x: u32) -> u8 { (x / 2).to(u8) };
low: u8 = half(100);
high: u8 = half(1000);
Wrapped: type = Int(0..=300.wrap(u8));
wrapped: Wrapped = 45;
",
    );
    assert_eq!(
        fixture.reported(&file),
        [
            (errors::INT_DOES_NOT_FIT, "value does not fit the type `u8`".to_owned()),
            (
                errors::COMPTIME_TRAP,
                "computing this at compile time failed: the value does not fit the type it is narrowed to"
                    .to_owned()
            ),
            (errors::INT_DOES_NOT_FIT, "value does not fit the type `Wrapped`".to_owned()),
        ]
    );
}

#[test]
fn an_impl_without_an_interface_adds_methods() {
    let fixture = Fixture::new("impls-inherent");
    let file = fixture.root.write(
        "pairs.vun",
        "\
Pair := ($T: type) -> type { struct { a: T, b: T } };
#[impl(Pair)]
pair_methods := ($T: type) -> type { struct { first := (p: Pair(T)) -> T { p.a }; } };
first := (p: Pair(u8)) -> bool { p.first() };
mapped := (r: Result(u8, u8)) -> Result(u8, bool) { r.map_err((e: u8) -> bool { true }) };
",
    );
    assert_eq!(fixture.reported(&file), [(errors::TYPES_MISMATCH, "expected `bool`, found `u8`".to_owned())]);
}

#[test]
fn a_value_that_is_no_function_is_called_through_its_witness() {
    let fixture = Fixture::new("impls-holed");
    let file = fixture.root.write(
        "counter.vun",
        "\
Counter := struct { calls: usize };
#[impl(lang.ops.Call(_, usize)(Counter))]
counter_call := ($Params: type) -> type { struct {
    call := (self: &mut Counter, args: Params) -> usize {
        self.calls += 1;
        lang.meta.items(Params).len()
    };
} };
count := () -> usize {
    mut c: Counter = .{ .calls = 0 };
    (c(1, true) + c() + c(2) + c.calls).wrap(usize)
};
Six: type = Int(0..=count());
fits: Six = 6;
over: Six = 7;
wrong := () -> bool {
    mut c: Counter = .{ .calls = 0 };
    c(1)
};
",
    );
    assert_eq!(
        fixture.reported(&file),
        [
            (errors::INT_DOES_NOT_FIT, "value does not fit the type `Six`".to_owned()),
            (errors::TYPES_MISMATCH, "expected `bool`, found `u64`".to_owned()),
        ]
    );
}

#[test]
fn a_method_lookup_makes_no_witness_of_an_impl_that_cannot_have_it() {
    let fixture = Fixture::new("impls-pruned");
    let file = fixture.root.write(
        "marks.vun",
        "\
Shape := interface(($Self: type) -> type { struct {
    area: (self: &Self) -> u64 = _;
}});
Mark := interface(($Self: type) -> type { struct {
    tag: (self: &Self) -> u8 = _;
}});
Square := struct { side: u16 };
#[impl(Shape(Square))]
square_shape := struct { area := (self: &Square) -> u64 { self.side }; };
#[impl(Mark(Square))]
square_mark := struct { tag := (self: &Square) -> u8 { 1 }; };
measure := (s: Square) -> u64 { s.area() };
",
    );
    let mut session = fixture.session();
    let loaded = session.load(&file).unwrap();
    let measure = session.functions(loaded).pop().expect("the last function is measure");
    assert!(session.db.get::<Typed>(&Instantiation::plain(measure)).is_some());
    let here = session.file_ref(loaded);
    let made = session
        .db
        .computed::<Resolved>()
        .into_iter()
        .filter(|(candidate, _)| matches!(&candidate.source, Source::Impl(decl) if decl.decl.file == here))
        .count();
    assert_eq!(made, 1, "only the impl declaring `area` is made");
}
