//! Mutation at compile time (spec 03, section 7): exclusive borrows change what they reach, in a
//! body and across calls, which take what a loan reaches and give it back changed.

use crate::common::Fixture;
use vun_diag::errors;

#[test]
fn exclusive_borrows_change_what_they_reach() {
    let fixture = Fixture::new("comptime-mut");
    let file = fixture.root.write(
        "mut.vun",
        "\
S := struct { a: u32, b: u32 };
bump := (x: &mut u32) { x.* +%= 1; };
local := () -> u32 { mut n: u32 = 1; r := &mut n; r.* = 4; n };
twice := () -> u32 { mut n: u32 = 5; bump(&mut n); bump(&mut n); n };
field := () -> u32 { mut s := S{ .a = 1, .b = 2 }; bump(&mut s.b); s.b };
first := (s: &mut S) -> &mut u32 { &mut s.a };
through := () -> u32 { mut s := S{ .a = 1, .b = 2 }; first(&mut s).* = 9; s.a };
looped := () -> u32 { mut n: u32 = 0; for _ in 0..4 { bump(&mut n); } n };
four: Int(local()..=local()) = 4;
seven: Int(twice()..=twice()) = 7;
three: Int(field()..=field()) = 3;
nine: Int(through()..=through()) = 9;
counted: Int(looped()..=looped()) = 4;
wrong: Int(twice()..=twice()) = 6;
Plan := struct {
    steps: u32,
    step := (self: &mut Plan) -> u32 { self.steps +|= 1; self.steps };
};
recorded := () -> u32 { mut p := Plan{ .steps = 0 }; p.step(); p.step(); p.step() };
three_steps: Int(recorded()..=recorded()) = 3;
numbers := () -> Iter(u32) { yield 10; yield 20; };
next_of := ($F: Iter(u32), it: &mut F) -> u32 {
    match it.resume() {
        .yielded(x) => x,
        .done(_) => 0,
    }
};
second := () -> u32 {
    mut it := numbers();
    next_of(&mut it);
    x := next_of(&mut it);
    next_of(&mut it);
    it.end();
    x
};
twenty: Int(second()..=second()) = 20;
",
    );
    // Each annotation holds exactly the value the call evaluates to, but `wrong`: methods on
    // `&mut self` and frames lent to a function change what they reach too.
    let found = fixture.reported(&file);
    let ids: Vec<_> = found.iter().map(|(id, _)| *id).collect();
    assert_eq!(ids, [errors::INT_DOES_NOT_FIT], "{found:?}");
}

#[test]
fn methods_with_compile_time_parameters_change_their_receiver() {
    let fixture = Fixture::new("comptime-mut-generic");
    let file = fixture.root.write(
        "generic.vun",
        "\
P := struct {
    n: u32,
    add := (self: &mut P, $T: type, x: T) { self.n +|= 1; };
};
written := () -> u32 { mut p := P{ .n = 0 }; p.add(u32, 1); p.n };
inferred := () -> u32 { mut p := P{ .n = 0 }; p.add(1); p.add(2); p.n };
one: Int(written()..=written()) = 1;
two: Int(inferred()..=inferred()) = 2;
",
    );
    // The receiver is lent, not copied, though the method's type depends on `T`.
    assert_eq!(fixture.check(&file), []);
}

#[test]
fn a_mut_binding_is_read_at_run_time_only() {
    let fixture = Fixture::new("comptime-mut-static");
    let file = fixture.root.write(
        "statics.vun",
        "\
mut count: u8 = 1;
fixed: u8 = 1;
read := () -> u8 { unsafe { count } };
known: Int(fixed..=fixed) = 2;
unknown: Int(read()..=read()) = 2;
",
    );
    // `count` holds 1 only until something writes it: it has no compile-time value, so
    // `unknown` has no type to check 2 against, where `known` does.
    assert_eq!(fixture.check(&file), [errors::INT_DOES_NOT_FIT]);
}

#[test]
fn a_lent_parameter_is_lent_on() {
    let fixture = Fixture::new("comptime-mut-lent-on");
    let file = fixture.root.write(
        "lent.vun",
        "\
bump := (x: &mut u32) { x.* +|= 1; };
twice := (x: &mut u32) { bump(x); bump(x); };
run := () -> u32 { mut n: u32 = 0; twice(&mut n); n };
two: Int(run()..=run()) = 2;
wrong: Int(run()..=run()) = 1;
",
    );
    // `twice` passes on the borrow it was lent, and what `bump` writes comes back through both.
    assert_eq!(fixture.check(&file), [errors::INT_DOES_NOT_FIT]);
}

#[test]
fn loans_nested_in_arguments_change_what_they_reach() {
    let fixture = Fixture::new("comptime-mut-nested");
    let file = fixture.root.write(
        "nested.vun",
        "\
Counter := struct { n: &mut u32 };
Both := struct { a: &mut u32, b: &mut u32 };
Outer := struct { c: Counter, k: u32 };
Slot := enum { full(&mut u32), empty };
bump := (c: Counter) { c.n.* +|= 1; };
pair := (a: (&mut u32, u32)) { a.0.* +|= a.1; };
both := (p: Both) { p.a.* +|= 1; p.b.* +|= 10; };
deep := (o: Outer) { bump(o.c); bump(o.c); };
fill := (s: Slot) { match s { .full(r) => { r.* +|= 3; }, .empty => {} } };
first := (c: Counter) -> &mut u32 { c.n };
wrap := (n: &mut u32) -> Counter { Counter{ .n = n } };
one := () -> u32 { mut n: u32 = 1; bump(Counter{ .n = &mut n }); n };
tuple := () -> u32 { mut n: u32 = 1; pair((&mut n, 5)); n };
fields := () -> u32 { mut x: u32 = 0; mut y: u32 = 0; both(Both{ .a = &mut x, .b = &mut y }); (x + y).sat(u32) };
levels := () -> u32 { mut n: u32 = 0; deep(Outer{ .c = Counter{ .n = &mut n }, .k = 0 }); n };
payload := () -> u32 { mut n: u32 = 4; fill(Slot.full(&mut n)); n };
returned := () -> u32 { mut n: u32 = 0; first(Counter{ .n = &mut n }).* = 7; n };
escaped := () -> u32 { mut n: u32 = 0; wrap(&mut n).n.* = 9; n };
some := (n: &mut u32) -> ?&mut u32 { .some(n) };
via := () -> u32 { mut n: u32 = 0; match some(&mut n) { .some(r) => { r.* = 5; }, .none => {} } n };
two: Int(one()..=one()) = 2;
six: Int(tuple()..=tuple()) = 6;
eleven: Int(fields()..=fields()) = 11;
twice: Int(levels()..=levels()) = 2;
seven: Int(payload()..=payload()) = 7;
set: Int(returned()..=returned()) = 7;
nine: Int(escaped()..=escaped()) = 9;
maybe: Int(via()..=via()) = 5;
wrong_one: Int(one()..=one()) = 1;
wrong_tuple: Int(tuple()..=tuple()) = 5;
wrong_levels: Int(levels()..=levels()) = 1;
wrong_returned: Int(returned()..=returned()) = 8;
",
    );
    // Each annotation holds exactly the value its call evaluates to, but the `wrong` ones: a loan
    // nested in an aggregate argument is lent as storage past the arguments, followed, passed on
    // and returned as any other, and a loan nested in a result is rebased on the caller's.
    let found = fixture.reported(&file);
    let ids: Vec<_> = found.iter().map(|(id, _)| *id).collect();
    assert_eq!(ids, [errors::INT_DOES_NOT_FIT; 4], "{found:?}");
}
