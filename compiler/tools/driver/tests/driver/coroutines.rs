//! Coroutine functions, frames and `for p in e` (spec 06), at compile time.

use crate::support::Fixture;
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
            fit("u8"),
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
tick := (e: &std.task.Event) -> std.task.Async(never) { for { yield ..e.wait(); } };
count := (e: &std.task.Event) -> std.task.Async(u32) { yield ..e.wait(); 3 };
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
    defer c.end();
    mut d := count(e);
    defer d.end();
    r := yield ..std.task.select(.{ &mut c, &mut d });
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

#[test]
fn iterators_zip_chain_and_fold() {
    let fixture = Fixture::new("coroutines-combinators");
    let file = fixture.root.write(
        "combinators.vun",
        "\
evens := (limit: u32) -> Iter(u32) { for i in 0..limit { if i % 2 == 0 { yield i; } } };
ones := (n: u32) -> Iter(u8) { for _ in 0..n { yield 1; } };
chained := () -> usize { evens(4).chain(evens(6)).count() };
summed := () -> u32 { evens(10).fold(0, (acc: u32, x: u32) -> u32 { (acc + x).sat(u32) }) };
zipped := () -> usize { evens(10).zip(ones(3)).count() };
five: Int(chained()..=chained()) = 5;
twenty: Int(summed()..=summed()) = 20;
three: Int(zipped()..=zipped()) = 3;
wrong: Int(zipped()..=zipped()) = 5;
",
    );
    // Each count and sum is exact at compile time; `zip` stops with the shorter iterator.
    assert_eq!(fixture.check(&file), [errors::INT_DOES_NOT_FIT]);
}

#[test]
fn a_frame_ended_at_a_yield_runs_only_what_is_pending_there() {
    let fixture = Fixture::new("coroutines-cancel");
    let file = fixture.root.write(
        "cancel.vun",
        "\
count := (e: &std.task.Event) -> std.task.Async(u32) { yield ..e.wait(); 3 };
held := (e: &std.task.Event) -> std.task.Async(u32) {
    mut c := count(e);
    yield ..e.wait();
    c.end();
    3
};
deferred := (e: &std.task.Event) -> std.task.Async(u32) {
    mut c := count(e);
    defer c.end();
    yield ..e.wait();
    3
};
",
    );
    // Ending `held` at its `yield` would leave `c` held; `deferred` ends it there.
    let found = fixture.reported(&file);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].0, errors::PLACE_LINEAR_LEAK);
}

#[test]
fn a_frame_ended_at_a_yield_runs_its_pending_defers() {
    let source = "\
start := std.os.start(main);
count := (hits: &mut u8) -> Iter(u8) {
    defer hits.* = 7;
    yield 1;
    yield 2;
};
main := (sys: &mut std.os.System) -> std.os.Exit {
    mut hits: u8 = 0;
    {
        mut f := count(&mut hits);
        _ := f.resume();
        f.end();
    }
    std.os.Exit.of(hits % 126)
};
";
    assert_eq!(crate::backend::status("coroutines-frame-end", source), Some(7));
}
