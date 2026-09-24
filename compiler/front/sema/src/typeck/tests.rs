use crate::check::Checked;
use crate::tests::packages;

/// The messages checking `app` reports, in source order.
fn reported(app: &str) -> Vec<String> {
    let (db, file) = packages(app);
    let reports = db.get::<Checked>(&file).unwrap();
    reports.sorted().iter().map(|d| format!("{}: {}", d.id, d.message)).collect()
}

#[test]
fn bodies_are_typed_from_parameters_locals_and_results() {
    let app = "\
cube := (x: u8) -> u8 { x * x * x };
add := (a: u8, b: u8) -> u8 { a + b };
call := () -> u8 { add(1, 300) };
local := () { y: u8 = 256; };
empty := () -> u8 { };
early := (x: u8) -> u8 { return x; };
fits := (x: u8) -> Int(0..=510) { add(x, x) + x };
";
    assert_eq!(
        reported(app),
        [
            "int.does_not_fit: value does not fit the type `u8`",
            "int.does_not_fit: value does not fit the type `u8`",
            "int.does_not_fit: value does not fit the type `Int(0..=255)`",
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
    assert_eq!(reported(app), ["int.width_cap: this intermediate needs 192 bits, past the cap of 128"]);
}

#[test]
fn function_types_are_values() {
    let app = "\
f: (u8) -> u8 = 3;
g: (u8) -> u8 = (x: u8) -> Int(0..=9) { 0 };
h: (u8) -> u8 = (x: u8) -> u8 { x };
";
    assert_eq!(
        reported(app),
        [
            "types.mismatch: expected `(u8) -> u8`, found `Int(3..=3)`",
            "types.mismatch: expected `(u8) -> u8`, found `(Int(0..=255)) -> Int(0..=9)`",
        ]
    );
}

#[test]
fn nominal_types_are_distinct_and_named_by_their_declaration() {
    let app = "\
NodeId := struct { raw: u8 };
TypeId := struct { raw: u8 };
type_of := (n: NodeId) -> TypeId { n };
same := (n: NodeId) -> NodeId { n };
";
    assert_eq!(reported(app), ["types.mismatch: expected `TypeId`, found `NodeId`"]);
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
        reported(app),
        [
            "int.does_not_fit: value does not fit the type `u8`",
            "int.does_not_fit: value does not fit the type `Int(0..=255)`",
            "types.no_member: no member `z` in `Point`",
            "int.does_not_fit: value does not fit the type `u8`",
        ]
    );
}
