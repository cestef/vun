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

#[test]
fn omitted_comptime_arguments_are_inferred_from_the_others() {
    let fixture = Fixture::new("generics-inferred");
    let file = fixture.root.write(
        "inferred.ov",
        "\
max := ($T: type, a: T, b: T) -> T { if a > b { a } else { b } };
E := enum { fs(u8), oom };
apply := ($A: type, $B: type, x: A, f: (A) -> B) -> B { f(x) };
joined := (x: u8) -> u8 { max(3, x) };
written := (x: u8) -> u8 { max(u8, 3, x) };
constructor := (x: u8) -> E { apply(x, E.fs) };
wrong := (x: u8) -> bool { max(3, x) };
first := ($T: type, o: ?T) -> ?T { o };
none := () { first(.none); };
",
    );
    let ids: Vec<_> = fixture.reported(&file).into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, [errors::TYPES_MISMATCH, errors::TYPES_CANNOT_INFER]);
}

#[test]
fn callbacks_take_functions_closures_and_constructors() {
    let fixture = Fixture::new("generics-callables");
    let file = fixture.root.write(
        "callables.ov",
        "\
E := enum { bad(u8) };
lit := (o: ?u8) -> ?u16 { o.map((x: u8) -> u16 { x + 1 }) };
clo := (o: ?u8, k: u8) -> ?u16 { o.map([k] (x: u8) -> u16 { x + k }) };
ctor := (r: Result(u8, u8)) -> Result(u8, E) { r.map_err(E.bad) };
wrong := (o: ?u8, k: u8) -> ?bool { o.map([k] (x: u8) -> u16 { x + k }) };
",
    );
    let ids: Vec<_> = fixture.reported(&file).into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, [errors::TYPES_MISMATCH]);
}
