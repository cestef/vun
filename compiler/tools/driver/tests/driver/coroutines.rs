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

#[test]
fn executors_drive_frames_through_their_own_resume_and_end() {
    let fixture = Fixture::new("coroutines-executors");
    let file = fixture.root.write(
        "executors.vun",
        "\
tick := (e: &std.task.Event) -> std.task.Async(never) { for { yield .{ .event = e }; } };
count := (e: &std.task.Event) -> std.task.Async(u32) { yield .{ .event = e }; 3 };
go := (e: &std.task.Event) -> never {
    mut a := tick(e);
    mut b := tick(e);
    std.task.run(.{ &mut a, &mut b })
};
once := (e: &std.task.Event) -> u32 {
    mut c := count(e);
    n := std.task.block_on(&mut c);
    c.end();
    n
};
either := (e: &std.task.Event) -> std.task.Async(bool) {
    mut c := count(e);
    mut d := count(e);
    r := yield ..std.task.select(.{ &mut c, &mut d });
    c.end();
    d.end();
    match r { .first(_) => true, .second(_) => false }
};
kept := (e: &std.task.Event) -> u32 {
    mut c := count(e);
    std.task.block_on(&mut c)
};
",
    );
    let ids: Vec<_> = fixture.reported(&file).into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, [errors::PLACE_LINEAR_LEAK], "`kept` never ends its frame");
}

#[test]
fn iterators_adapt_through_methods_of_every_frame() {
    let fixture = Fixture::new("coroutines-adapters");
    let file = fixture.root.write(
        "adapters.vun",
        "\
evens := (limit: u32) -> Iter(u32) { for i in 0..limit { if i % 2 == 0 { yield i; } } };
total := () -> usize { evens(10).take(3).count() };
second := (xs: &[4]u8) -> u8 {
    mut s: u8 = 0;
    for (i, x) in xs.enumerate() { if i == 1 { s = x.*; } }
    s
};
kept := () -> usize { evens(8).map((x: u32) -> u64 { x * 2 }).filter((x: &u64) -> bool { x.* > 2 }).count() };
wrong := () -> bool { evens(4).count() };
",
    );
    let ids: Vec<_> = fixture.reported(&file).into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, [errors::TYPES_MISMATCH]);
}
