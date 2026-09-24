//! Compile-time evaluation through the whole front end, over the toolchain's `lang` and `std`.

use crate::common::Fixture;
use overt_diag::errors;
use overt_span::SourceMap;

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

#[test]
fn loops_and_assignments_run_at_compile_time() {
    let fixture = Fixture::new("comptime-loops");
    let file = fixture.root.write(
        "loops.ov",
        "\
sum := ($n: u8) -> u16 {
    mut total: u16 = 0;
    mut i: u8 = 0;
    for i < n { i += 1; total += i; }
    total
};
root := ($n: u8) -> u8 {
    mut i: u8 = 0;
    'search: for {
        if i * i > n { break 'search i; }
        i += 1;
    }
};
wrap := ($n: u8) -> u8 { mut x: u8 = n; x +%= 250; x };
early := ($n: u8) -> u8 { if n > 3 { return 1; } 2 };
Sum := Int(0..=sum(4));
Root := Int(0..=root(20));
Wrap := Int(0..=wrap(10));
Early := Int(0..=early(9));
sum_fits: Sum = 10;
sum_over: Sum = 11;
root_over: Root = 6;
wrap_over: Wrap = 5;
early_over: Early = 2;
",
    );
    let messages: Vec<String> = fixture.reported(&file).into_iter().map(|(_, m)| m).collect();
    assert_eq!(
        messages,
        ["Sum", "Root", "Wrap", "Early"].map(|t| format!("value does not fit the type `{t}`"))
    );
}

#[test]
fn a_compile_time_trap_is_reported_at_its_root_with_the_calls_it_fired_in() {
    let fixture = Fixture::new("comptime-trap");
    let file = fixture.root.write(
        "trap.ov",
        "\
half := ($n: Int(0..=9)) -> Int(0..=10) { 10 / n };
twice := ($n: Int(0..=9)) -> Int(0..=20) { half(n) + half(n) };
Small := Int(0..=twice(0));
",
    );
    let mut session = fixture.session();
    let file = session.load(&file).unwrap();
    session.check(file);
    let reported = session.diag.sorted();
    let [trap] = reported.as_slice() else { panic!("{reported:#?}") };
    assert_eq!(trap.id, errors::COMPTIME_TRAP);
    let line = |origin| {
        let span = session.origins.source_span(origin);
        session.sources.file(span.file).line_col(span.range.lo).line
    };
    assert_eq!(trap.labels.iter().map(|l| line(l.origin)).collect::<Vec<_>>(), [1]);
    assert_eq!(trap.trace.iter().map(|t| line(t.origin)).collect::<Vec<_>>(), [2, 3]);
}

#[test]
fn type_syntax_and_sequences_are_typed() {
    let fixture = Fixture::new("comptime-sequences");
    let file = fixture.root.write(
        "sequences.ov",
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
            "value does not fit the type `Int(0..=255)`",
            "expected `?u16`, found `Option(Int(0..=255))`",
        ]
    );
}

#[test]
fn compile_time_calls_build_and_read_values() {
    let fixture = Fixture::new("comptime-values");
    let file = fixture.root.write(
        "values.ov",
        "\
Pair := struct { a: u8, b: u8 = 5 };
make := ($x: u8) -> Pair { .{ .a = x } };
swap := ($p: Pair) -> Pair { Pair{ .a = p.b, .b = p.a } };
Big := Int(0..=swap(make(3)).a);
big: Big = 6;
second := ($xs: [3]u8) -> u8 { xs[1] };
Second := Int(0..=second([1, 9, 2]));
two: Second = 10;
Register := struct { name: &[u8], offset: u16 };
regs: &[Register] = &[.{ .name = \"cr1\", .offset = 0 }, .{ .name = \"arr\", .offset = 0x2c }];
Offset := Int(0..=regs[1].offset);
offset: Offset = 45;
",
    );
    let fit = |ty: &str| (errors::INT_DOES_NOT_FIT, format!("value does not fit the type `{ty}`"));
    assert_eq!(fixture.reported(&file), [fit("Big"), fit("Second"), fit("Offset")]);
}
