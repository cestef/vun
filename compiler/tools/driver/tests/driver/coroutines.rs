//! Coroutine functions, frames and `for p in e` (spec 06), at compile time.

use crate::common::Fixture;
use vun_diag::errors;

#[test]
fn frames_yield_to_for_and_to_delegation() {
    let fixture = Fixture::new("coroutines-frames");
    let file = fixture.root.write(
        "frames.vun",
        "\
three := () -> Iter(u8) { yield 1; yield 2; yield 3; };
sum := () -> u16 { mut n: u16 = 0; for x in three() { n += x; } n };
six: Int(0..=sum()) = 7;
twice := () -> Iter(u8) { yield ..three(); yield ..three() };
total := () -> u16 { mut n: u16 = 0; for x in twice() { n += x; } n };
twelve: Int(0..=total()) = 13;
wide := () -> Iter(u8) { yield 300; };
ends := () -> Iter(u8) { yield 1; 5 };
frame := () -> u8 { three() };
",
    );
    let found =
        |expected: &str, ty: &str| (errors::TYPES_MISMATCH, format!("expected `{expected}`, found `{ty}`"));
    let fit = |ty: &str| (errors::INT_DOES_NOT_FIT, format!("value does not fit the type `{ty}`"));
    assert_eq!(
        fixture.reported(&file),
        [
            fit("Int(0..=sum())"),
            fit("Int(0..=total())"),
            fit("Int(0..=255)"),
            found("()", "Int(5..=5)"),
            found("u8", "three.Frame"),
        ]
    );
}

#[test]
fn bounds_arrays_and_slices_iterate_through_lang() {
    let fixture = Fixture::new("coroutines-lang");
    let file = fixture.root.write(
        "iterate.vun",
        "\
range := () -> u16 { mut n: u16 = 0; for i in 0..5 { n += i; } n };
ten: Int(0..=range()) = 11;
array := () -> u16 { xs: [3]u8 = [4, 5, 6]; mut n: u16 = 0; for x in xs { n += x; } n };
fifteen: Int(0..=array()) = 16;
slice := () -> u16 { xs: [3]u8 = [1, 2, 3]; s: &[u8] = &xs; mut n: u16 = 0; for x in s { n += x.*; } n };
six: Int(0..=slice()) = 7;
",
    );
    let reported: Vec<_> = fixture.reported(&file).into_iter().map(|(id, _)| id).collect();
    assert_eq!(reported, [errors::INT_DOES_NOT_FIT, errors::INT_DOES_NOT_FIT, errors::INT_DOES_NOT_FIT]);
}
