//! Markers of function types (spec 07, sections 3 and 4): accepted by the relation the table
//! `fn_markers` gives each, and `#[panic.never]` checked on the body.

use crate::common::Fixture;
use vun_diag::errors;

#[test]
fn functions_are_accepted_by_the_relation_of_their_markers() {
    let fixture = Fixture::new("markers-accept");
    let file = fixture.root.write(
        "accept.vun",
        "\
plain := (x: u8) -> u8 { x };
risky := unsafe (x: u8) -> u8 { x };
#[panic.never]
calm := (x: u8) -> u8 { x };
#[stack.max(32)]
small := (x: u8) -> u8 { x };
#[stack.max(128)]
large := (x: u8) -> u8 { x };
duty := (f: unsafe (u8) -> u8) -> u8 { unsafe f(1) };
safe := (f: (u8) -> u8) -> u8 { f(1) };
promise := (f: #[panic.never] (u8) -> u8) -> u8 { f(1) };
bound := (f: #[stack.max(64)] (u8) -> u8) -> u8 { f(1) };
a := () -> u8 { duty(plain) };
b := () -> u8 { safe(risky) };
c := () -> u8 { promise(calm) };
d := () -> u8 { promise(plain) };
e := () -> u8 { bound(small) };
g := () -> u8 { bound(large) };
",
    );
    let ids: Vec<_> = fixture.reported(&file).into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, [errors::TYPES_MISMATCH, errors::TYPES_MISMATCH, errors::TYPES_MISMATCH]);
}

#[test]
fn functions_that_never_trap_contain_no_trap_site() {
    let fixture = Fixture::new("markers-never");
    let file = fixture.root.write(
        "never.vun",
        "\
#[panic.never]
halve := (x: u32) -> u32 { x / 2 };
#[panic.never]
divide := (x: u32, n: u32) -> u32 { x / n };
#[panic.never]
first := (xs: [4]u8, i: Int(0..=3)) -> u8 { xs[i] };
#[panic.never]
any := (xs: [4]u8, i: u8) -> u8 { xs[i] };
#[panic.never]
calls := (x: u32) -> u32 { halve(x) };
#[panic.never]
bumps := (x: u8) -> u8 { mut y := x; y += 1; y };
",
    );
    let ids: Vec<_> = fixture.reported(&file).into_iter().map(|(id, _)| id).collect();
    assert_eq!(
        ids,
        [errors::PANIC_NEVER_VIOLATED, errors::PANIC_NEVER_VIOLATED, errors::PANIC_NEVER_VIOLATED]
    );
}
