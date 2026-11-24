//! `$for` and `$if` in type bodies and computed names (spec 03, section 2).

use crate::support::Fixture;
use vun_diag::errors;

#[test]
fn generated_members_are_typed_and_evaluated() {
    let fixture = Fixture::new("staged-members");
    let file = fixture.root.write(
        "peripheral.vun",
        "\
Register := struct { name: &[u8], reset: u32 };
Block := ($registers: &[Register]) -> type {
    $Regs := struct {
        $for r in registers { $(r.name): u32, }
        $for r in registers { $(\"reset_\" ++ r.name) := (self: &mut Regs) -> u32 { r.reset }; }
        $if registers.len() > 1 { many := 1; } else { few := 1; }
    };
    Regs
};
Timer: type = Block(&[.{ .name = \"cr1\", .reset = 5 }, .{ .name = \"arr\", .reset = 9 }]);
method := (t: &mut Timer) -> bool { t.reset_arr() };
field := (t: Timer) -> bool { t.cr1 };
",
    );
    let found = |ty: &str| (errors::TYPES_MISMATCH, format!("expected `bool`, found `{ty}`"));
    assert_eq!(fixture.reported(&file), [found("u32"), found("u32")]);
}

#[test]
fn staged_statements_are_typed_per_step() {
    let fixture = Fixture::new("staged-statements");
    let file = fixture.root.write(
        "fields.vun",
        "\
Config := struct { baud: u32, parity: bool };
copy := (c: Config) -> Config {
    mut out := c;
    $for f in std.meta.fields(Config) { out.$(f.name) = c.$(f.name); }
    out
};
words := (c: Config) {
    $for f in std.meta.fields(Config) { _: u32 = c.$(f.name); }
};
taken := () {
    $if 1 > 2 { _: bool = 3; } else { _: bool = 4; }
};
",
    );
    let found =
        |ty: &str, expected: &str| (errors::TYPES_MISMATCH, format!("expected `{expected}`, found `{ty}`"));
    assert_eq!(fixture.reported(&file), [found("bool", "u32"), found("Int(4..=4)", "bool")]);
}

#[test]
fn staged_statements_run_unrolled() {
    let fixture = Fixture::new("staged-run");
    let file = fixture.root.write(
        "dot.vun",
        "\
dot := ($N: u32, a: &[N]u32, b: &[N]u32) -> u64 {
    mut acc: u64 = 0;
    $for i in 0..N { acc += a[i] * b[i]; }
    acc
};
fits: Int(0..=dot(3, &[1, 2, 3], &[4, 5, 6])) = 32;
over: Int(0..=dot(3, &[1, 2, 3], &[4, 5, 6])) = 33;
seven := () -> u8 { $if 1 > 2 { (1, 2) } else if 2 > 1 { 7 } else { 9 } };
eight: Int(0..=seven()) = 8;
sum := () -> u32 { mut s: u32 = 0; $for i in 0..3 { f := () -> u32 { i + 1 }; s += f(); } s };
seven_again: Int(0..=sum()) = 7;
",
    );
    let reported: Vec<_> = fixture.reported(&file).into_iter().map(|(id, _)| id).collect();
    assert_eq!(reported, [errors::INT_DOES_NOT_FIT; 3]);
}

#[test]
fn staged_branches_compare_types() {
    let fixture = Fixture::new("staged-same");
    let file = fixture.root.write(
        "same.vun",
        "\
width := ($T: type) -> u8 { $if lang.meta.same(type, T, u8) { 8 } else { 0 } };
eight: Int(width(u8)..=width(u8)) = 8;
none: Int(width(u16)..=width(u16)) = 0;
wrong: Int(width(u16)..=width(u16)) = 8;
",
    );
    // `lang.meta.same` tells whether two types are one; `wrong` has the value `none` has.
    assert_eq!(fixture.check(&file), [errors::INT_DOES_NOT_FIT]);
}
