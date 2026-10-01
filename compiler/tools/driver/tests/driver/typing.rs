//! Typing bodies against the toolchain's `lang` (spec 02 and 04): operators are calls of their
//! interfaces' functions, so what an expression's type is depends on `lang`'s impls.

use crate::common::Fixture;

/// The reports of checking `app` as a file of its own, as `id: message`, in source order.
fn reported(name: &str, app: &str) -> Vec<String> {
    let fixture = Fixture::new(name);
    let file = fixture.root.write("app.vun", app);
    fixture.reported(&file).into_iter().map(|(id, message)| format!("{id}: {message}")).collect()
}

#[test]
fn bodies_are_typed_from_parameters_locals_and_results() {
    let app = "\
cube := (x: u8) -> u8 { x * x * x };
add := (a: u8, b: u8) -> u8 { a + b };
call := () -> u8 { add(1, 300) };
local := () { _: u8 = 256; };
empty := () -> u8 { };
early := (x: u8) -> u8 { return x; };
fits := (x: u8) -> Int(0..=510) { add(x, x) + x };
";
    assert_eq!(
        reported("typing", app),
        [
            "int.does_not_fit: value does not fit the type `u8`",
            "int.does_not_fit: value does not fit the type `u8`",
            "int.does_not_fit: value does not fit the type `u8`",
            "int.does_not_fit: value does not fit the type `u8`",
            "types.mismatch: expected `u8`, found `()`",
        ]
    );
}

#[test]
fn run_time_intermediates_are_capped_and_compile_time_ones_are_not() {
    let app = "\
wide := (x: Int(0..1 << 64)) -> Int(0..1 << 64) { x * x * x + 1 };
free := ($n: Int(0..1 << 64)) -> Int(0..) { n * n * n };
";
    assert_eq!(
        reported("typing", app),
        ["int.width_cap: this intermediate needs 192 bits, past the cap of 128"]
    );
}

#[test]
fn fields_literals_and_methods_of_structs_are_typed() {
    let app = "\
Point := struct {
    x: u8,
    y: u8,
    origin := Point{ .x = 0, .y = 0 };
    norm := (p: Point) -> Int(0..=510) { p.x + p.y };
};
sum := (p: Point) -> u8 { p.x + p.y };
literal := () -> Point { Point{ .x = 300, .z = 1 } };
method := (p: Point) -> u8 { p.norm() };
constant := () -> Point { Point.origin };
";
    assert_eq!(
        reported("typing", app),
        [
            "int.does_not_fit: value does not fit the type `u8`",
            "int.does_not_fit: value does not fit the type `u8`",
            "types.no_member: no member `z` in `Point`",
            "int.does_not_fit: value does not fit the type `u8`",
        ]
    );
}

#[test]
fn expected_types_type_dot_names_literals_tuples_and_matches() {
    let app = "\
Color := enum { red, green, blue(u8) };
Point := struct { x: u8, y: u8 };
pick := (c: Color) -> u8 { match c { .red => 1, .green => 2, .blue(n) => n } };
bad := (c: Color) -> u8 { match c { .red => 1, .purple => 2, .green => 3, .blue(n) => n + 300 } };
make := () -> Color { .blue(3) };
wrong := () -> Color { .blue(300) };
unknown := () -> Color { .yellow };
anon := () -> Point { .{ .x = 1, .z = 2 } };
early := (x: u8) -> u8 { return x + 1; };
assign := () { mut y: u8 = 0; y = 256; };
pair := () -> (u8, u8) { (1, 2) };
wide := () -> (u8, u8) { (1, 300) };
test := (c: Color) -> u8 { if .blue(n) = c { n } else { 0 } };
";
    assert_eq!(
        reported("typing", app),
        [
            "types.no_member: no member `purple` in `Color`",
            "int.does_not_fit: value does not fit the type `u8`",
            "int.does_not_fit: value does not fit the type `u8`",
            "types.no_member: no member `yellow` in `Color`",
            "types.no_member: no member `z` in `Point`",
            "int.does_not_fit: value does not fit the type `u8`",
            "int.does_not_fit: value does not fit the type `u8`",
            "int.does_not_fit: value does not fit the type `u8`",
        ]
    );
}
