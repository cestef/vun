//! Control flow at compile time: branches, loops, early returns, traps and the steps calls spend.

use crate::common::Fixture;
use vun_diag::errors;
use vun_span::SourceMap;

#[test]
fn branches_run_at_compile_time_and_join_their_types() {
    let fixture = Fixture::new("comptime-branches");
    let file = fixture.root.write(
        "branches.vun",
        "\
max := ($a: Int(0..=100), $b: Int(0..=100)) -> Int(0..=100) { if a > b { a } else { b } };
pick := ($n: u8) -> u8 { if n == 1 and !(n > 5) or n == 7 { 10 } else { 20 } };
Low: type = Int(0..=max(3, 70));
Picked: type = Int(0..=pick(1) + pick(2));
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
            (errors::TYPES_MISMATCH, "expected `bool`, found `u8`".to_owned()),
        ]
    );
}

#[test]
fn loops_and_assignments_run_at_compile_time() {
    let fixture = Fixture::new("comptime-loops");
    let file = fixture.root.write(
        "loops.vun",
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
Sum: type = Int(0..=sum(4));
Root: type = Int(0..=root(20));
Wrap: type = Int(0..=wrap(10));
Early: type = Int(0..=early(9));
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
        "trap.vun",
        "\
half := ($n: Int(0..=9)) -> Int(0..=10) { 10 / n };
twice := ($n: Int(0..=9)) -> Int(0..=20) { half(n) + half(n) };
Small: type = Int(0..=twice(0));
",
    );
    let mut session = fixture.session();
    let file = session.load(&file).unwrap();
    session.check(file);
    let reported = session.diag.sorted(session.origins);
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
fn a_call_spends_the_steps_of_the_calls_it_makes() {
    let fixture = Fixture::new("comptime-fuel");
    let file = fixture.root.write(
        "fuel.vun",
        "\
count := ($n: u32) -> u32 { mut i: u32 = 0; for i < n { i += 1; } i };
#[comptime.fuel(50)]
small := ($n: u32) -> u32 { count(n) };
#[comptime.fuel(1000)]
large := ($n: u32) -> u32 { count(n) };
fits := small(3);
spent := small(30);
raised := large(30);
",
    );
    let reported: Vec<_> = fixture.reported(&file).into_iter().map(|(id, _)| id).collect();
    assert_eq!(reported, [errors::COMPTIME_EXHAUSTED]);
}

#[test]
fn early_returns_go_on_or_stop() {
    let fixture = Fixture::new("comptime-try");
    let file = fixture.root.write(
        "try.vun",
        "\
parse := ($x: u8) -> Result(u8, u8) { if x > 9 { .err(x) } else { .ok(x) } };
double := ($x: u8) -> Result(u16, u8) { v := parse(x)?; .ok(v + v) };
got := ($r: Result(u16, u8)) -> u16 { match r { .ok(v) => v, .err(e) => 1000 + e } };
eight: Int(0..=got(double(4))) = 9;
stopped: Int(0..=got(double(12))) = 1013;
some := ($o: ?u8) -> ?u8 { v := o?; .some(v) };
wrong := ($x: u8) -> Result(u16, u16) { v := parse(x)?; .ok(v) };
",
    );
    let found = fixture.reported(&file);
    let ids: Vec<_> = found.iter().map(|(id, _)| id.clone()).collect();
    assert_eq!(ids, [errors::INT_DOES_NOT_FIT, errors::INT_DOES_NOT_FIT, errors::TYPES_MISMATCH]);
    assert_eq!(found[2].1, "expected `Result(u16, u16)`, found `Result(never, u8)`");
}

#[test]
fn an_exported_value_with_none_is_reported_with_where_evaluating_it_stopped() {
    let fx = Fixture::new("comptime-export");
    let table = Fixture::manifest("table", 2, "0.1.0");
    // `h` leads nowhere: the test is written value first, `if .some(h) = x` reversed.
    fx.write(
        "table",
        &table,
        "\
Config := struct { a: ?u8 = .none };
build := ($c: Config) -> type { struct {
    #[link.export(\"table\")]
    table := fill(c);
} };
fill := ($c: Config) -> [2]u8 { mut t: [2]u8 = [0; 2]; if c.a = .some(h) { t[0] = h; } t };
",
    );
    let mut app = Fixture::manifest("app", 1, "0.1.0");
    app.deps.push(Fixture::local("table", "../table"));
    let dir = fx.write("app", &app, "built := table.build(.{ .a = .some(3) });\n");
    assert_eq!(
        fx.check(&Fixture::file(&dir, &app)),
        [errors::COMPTIME_EXPORT, errors::NAMES_UNRESOLVED, errors::NAMES_UNRESOLVED]
    );
}

#[test]
fn a_staged_test_binds_what_its_pattern_takes_in_the_branch_it_selects() {
    let fixture = Fixture::new("comptime-tests");
    let file = fixture.root.write(
        "tests.vun",
        "\
Parsed := enum { good(u8), bad(u8) };
read := ($x: u8) -> Parsed { if x > 9 { .bad(x) } else { .good(x) } };
Big := ($x: u8) -> type {
    $if .bad(n) = read(x) { Int(0..=n) } else { u8 }
};
total := ($x: u8) -> u16 {
    mut t: u16 = 0;
    $if .good(d) = read(x) { t = d + 1; } else { t = 100; }
    t
};
both := () -> u32 { total(3) + total(20) };
sum: Int(0..=both()) = 105;
big: Big(20) = 21;
small: Big(3) = 255;
",
    );
    let reported: Vec<_> = fixture.reported(&file).into_iter().map(|(id, _)| id).collect();
    // `both()` is 4 + 100 and `Big(20)` is `Int(0..=20)`; `Big(3)` is `u8`, which 255 fits.
    assert_eq!(reported, [errors::INT_DOES_NOT_FIT, errors::INT_DOES_NOT_FIT]);
}
