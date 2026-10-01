//! Compile-time evaluation through the whole front end, over the toolchain's `lang` and `std`.

use crate::common::Fixture;
use vun_diag::errors;

mod control;
mod embed;
mod reports;

#[test]
fn each_instantiation_of_a_type_function_is_its_own_type() {
    let fixture = Fixture::new("comptime-instances");
    let file = fixture.root.write(
        "pairs.vun",
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
            (errors::TYPES_MISMATCH, "expected `Pair(u16)`, found `Pair(u8)`".to_owned()),
            (errors::INT_DOES_NOT_FIT, "value does not fit the type `u8`".to_owned()),
        ]
    );
}

#[test]
fn operators_are_typed_by_their_lang_ops_interfaces() {
    let fixture = Fixture::new("comptime-operators");
    let file = fixture.root.write(
        "ops.vun",
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
fn type_syntax_and_sequences_are_typed() {
    let fixture = Fixture::new("comptime-sequences");
    let file = fixture.root.write(
        "sequences.vun",
        "\
first := (xs: [4]u8) -> u8 { xs[0] };
slice := (xs: &[u8]) -> u8 { xs[1] };
wide := (xs: [2]u16) -> u8 { xs[0] };
literal := () -> [3]u8 { [1, 2, 300] };
zeros := () -> [4]u8 { [0; 4] };
maybe := (x: ?u8) -> ?u8 { x };
other := (x: ?u8) -> ?u16 { x };
pointer := (p: *u8) -> *u8 { p };
",
    );
    let messages: Vec<String> = fixture.reported(&file).into_iter().map(|(_, m)| m).collect();
    assert_eq!(
        messages,
        [
            "value does not fit the type `u8`",
            "value does not fit the type `u8`",
            "expected `?u16`, found `Option(u8)`",
        ]
    );
}

#[test]
fn compile_time_calls_build_and_read_values() {
    let fixture = Fixture::new("comptime-values");
    let file = fixture.root.write(
        "values.vun",
        "\
Pair := struct { a: u8, b: u8 = 5 };
make := ($x: u8) -> Pair { .{ .a = x } };
swap := ($p: Pair) -> Pair { Pair{ .a = p.b, .b = p.a } };
Big: type = Int(0..=swap(make(3)).a);
big: Big = 6;
second := ($xs: [3]u8) -> u8 { xs[1] };
Second: type = Int(0..=second([1, 9, 2]));
two: Second = 10;
Register := struct { name: &[u8], offset: u16 };
regs: &[Register] = &[.{ .name = \"cr1\", .offset = 0 }, .{ .name = \"arr\", .offset = 0x2c }];
Offset: type = Int(0..=regs[1].offset);
offset: Offset = 45;
pairs := .{ Pair{ .a = 1 }, Pair{ .a = 7 } };
Seventh: type = Int(0..=pairs.1.a);
seventh: Seventh = 8;
",
    );
    let fit = |ty: &str| (errors::INT_DOES_NOT_FIT, format!("value does not fit the type `{ty}`"));
    assert_eq!(fixture.reported(&file), [fit("Big"), fit("Second"), fit("Offset"), fit("Seventh")]);
}

#[test]
fn places_are_read_and_written_in_parts() {
    let fixture = Fixture::new("comptime-places");
    let file = fixture.root.write(
        "places.vun",
        "\
Pair := struct { a: u32, b: u32 };
swap := ($p: Pair) -> Pair { mut q := p; q.a = p.b; q.b = p.a; q };
first := () -> u32 { mut xs: [3]u32 = [1, 2, 3]; xs[0] = 7; xs[0] };
past := () -> u32 { xs: [2]u32 = [1, 2]; i: u32 = 5; xs[i] };
swapped := swap(Pair{ .a = 1, .b = 2 });
a: Int(0..=swapped.a) = 3;
first_seven: Int(0..=first()) = 8;
end: Int(0..=past()) = 0;
repeated := () -> u32 { xs: [3]u32 = [7; 3]; xs[2] };
seven: Int(0..=repeated()) = 8;
",
    );
    let reported: Vec<_> = fixture.reported(&file).into_iter().map(|(id, _)| id).collect();
    let fit = || errors::INT_DOES_NOT_FIT;
    assert_eq!(reported, [fit(), fit(), errors::COMPTIME_TRAP, fit()]);
}

#[test]
fn patterns_test_and_bind() {
    let fixture = Fixture::new("comptime-patterns");
    let file = fixture.root.write(
        "patterns.vun",
        "\
Shape := enum { circle(u8), square(u8), none };
area := ($s: Shape) -> u16 { match s { .circle(r) => r, .square(w) => w * w, .none => 0 } };
unwrap := ($o: ?u8) -> u8 { if .some(v) = o { v } else { 0 } };
first := () -> u8 { (a, b) := (4, 5); a + b };
drain := () -> u8 { mut o: ?u8 = .some(3); mut n: u8 = 0; for .some(v) = o { n += v; o = .none; } n };
kind := ($x: u8) -> u8 { match x { 0..=9 => 1, 10 | 11 => 2, _ => 3 } };
sq: Int(0..=area(.square(5))) = 26;
some: Int(0..=unwrap(.some(7))) = 8;
pair: Int(0..=first()) = 10;
drained: Int(0..=drain()) = 4;
kinded: Int(0..=kind(11)) = 3;
",
    );
    let reported: Vec<_> = fixture.reported(&file).into_iter().map(|(id, _)| id).collect();
    assert_eq!(reported, [errors::INT_DOES_NOT_FIT; 5]);
}

#[test]
fn closures_carry_their_captures() {
    let fixture = Fixture::new("comptime-closures");
    let file = fixture.root.write(
        "closures.vun",
        "\
apply := () -> u64 { k: u32 = 3; add := [k] (v: u32) -> u64 { v + k }; add(4) };
seven: Int(0..=apply()) = 8;
",
    );
    let reported: Vec<_> = fixture.reported(&file).into_iter().map(|(id, _)| id).collect();
    assert_eq!(reported, [errors::INT_DOES_NOT_FIT]);
}

#[test]
fn locals_holding_compile_time_values_are_known_then() {
    let fixture = Fixture::new("comptime-locals");
    let file = fixture.root.write(
        "locals.vun",
        "\
math := struct { double := (x: u16) -> u32 { x * 2 }; };
four := () -> u32 { m := math; m.double(2) };
fits: Int(0..=four()) = 4;
over: Int(0..=four()) = 5;
narrow := () -> u32 { T := u8; y: T = 300; y };
",
    );
    // `m` holds a namespace and `T` a type, values existing at compile time only.
    assert_eq!(fixture.check(&file), [errors::INT_DOES_NOT_FIT, errors::INT_DOES_NOT_FIT]);
}
