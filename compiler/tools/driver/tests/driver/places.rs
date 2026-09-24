//! Place states (spec 05, sections 2 and 3): reads need `init`, `&out` initializes, parts are
//! tracked, linear values move, and `defer` runs where its operand is still held.

use crate::common::Fixture;
use vun_diag::errors;

#[test]
fn place_states_follow_writes_and_borrows() {
    let fixture = Fixture::new("places-states");
    let file = fixture.root.write(
        "places.vun",
        "\
fill := (h: &out u8) { h.* = 1; };
filled := () -> u8 { mut x: u8; fill(&out x); x };
parts := () -> u8 { mut buf: [4]u8; buf[1] = 2; buf[1] };
other := () -> u8 { mut buf: [4]u8; buf[1] = 2; buf[0] };
half := (flag: bool) -> u8 { mut y: u8; if flag { y = 1; } else { } y };
partly := (flag: bool, h: &out u8) { if flag { h.* = 1; } };
",
    );
    let ids: Vec<_> = fixture.reported(&file).into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, [errors::PLACE_READ_UNINIT, errors::PLACE_AMBIGUOUS_JOIN, errors::PLACE_OUT_UNWRITTEN]);
}

#[test]
fn linear_values_move_and_must_be_consumed() {
    let fixture = Fixture::new("places-linear");
    let file = fixture.root.write(
        "linear.vun",
        "\
File := #[kind.linear] struct { fd: u32 };
open := () -> File { .{ .fd = 3 } };
close := (f: File) { .{ .fd = _ } := f; };
size := (f: &File) -> u32 { f.fd };
twice := () { f := open(); close(f); close(f); };
leak := () -> u32 { f := open(); size(&f) };
field := () -> u32 { f := open(); n := f.fd; close(f); n };
dropped := () { open(); };
",
    );
    let ids: Vec<_> = fixture.reported(&file).into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, [errors::PLACE_READ_UNINIT, errors::PLACE_LINEAR_LEAK, errors::PLACE_LINEAR_LEAK]);
}

#[test]
fn deferred_statements_run_on_the_exits_still_holding_their_operand() {
    let fixture = Fixture::new("places-defer");
    let file = fixture.root.write(
        "defer.vun",
        "\
File := #[kind.linear] struct { fd: u32 };
open := () -> File { .{ .fd = 3 } };
close := (f: File) { .{ .fd = _ } := f; };
size := (f: &File) -> u32 { f.fd };
closed := () { f := open(); defer close(f); };
early := (flag: bool) -> u32 { f := open(); defer close(f); if flag { return 1; } 2 };
handed := () -> File { f := open(); defer close(f); f };
looped := () { for { f := open(); defer close(f); break; } };
either := (flag: bool) { f := open(); defer close(f); if flag { close(f); } };
inner := () -> u32 { f := open(); { defer close(f); } size(&f) };
",
    );
    let ids: Vec<_> = fixture.reported(&file).into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, [errors::PLACE_AMBIGUOUS_JOIN, errors::PLACE_READ_UNINIT]);
}

#[test]
fn linear_values_are_consumed_before_their_scope_ends() {
    let fixture = Fixture::new("places-scopes");
    let file = fixture.root.write(
        "scopes.vun",
        "\
File := #[kind.linear] struct { fd: u32 };
open := () -> File { .{ .fd = 3 } };
close := (f: File) { .{ .fd = _ } := f; };
inner := () -> u32 { { f := open(); } 1 };
looped := () { for i in 0..3 { f := open(); } };
skipped := (flag: bool) { for { f := open(); if flag { continue; } close(f); break; } };
handed := () -> File { g := { f := open(); f }; g };
closed := () { { f := open(); close(f); } };
",
    );
    let found = fixture.reported(&file);
    let lines: Vec<_> = found.iter().map(|(id, _)| id.clone()).collect();
    // Each at the end of the block holding `f`: the inner block, the loop body, the `continue`.
    assert_eq!(
        lines,
        [errors::PLACE_LINEAR_LEAK, errors::PLACE_LINEAR_LEAK, errors::PLACE_LINEAR_LEAK],
        "{found:?}"
    );
    assert!(found[0].1.contains("before its scope ends"), "{found:?}");
}
