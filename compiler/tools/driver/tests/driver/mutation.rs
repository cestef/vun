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
looped := () -> u32 { mut n: u32 = 0; for i in 0..4 { bump(&mut n); } n };
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
    let ids: Vec<_> = found.iter().map(|(id, _)| id.clone()).collect();
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
