//! `$for` and `$if` in type bodies and computed names (spec 03, section 2).

mod common;

use common::Fixture;
use overt_diag::errors;

#[test]
fn generated_members_are_typed_and_evaluated() {
    let fixture = Fixture::new("staged-members");
    let file = fixture.root.write(
        "peripheral.ov",
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
Timer := Block(&[.{ .name = \"cr1\", .reset = 5 }, .{ .name = \"arr\", .reset = 9 }]);
method := (t: &mut Timer) -> bool { t.reset_arr() };
field := (t: Timer) -> bool { t.cr1 };
",
    );
    let found = |ty: &str| (errors::TYPES_MISMATCH, format!("expected `bool`, found `{ty}`"));
    assert_eq!(fixture.reported(&file), [found("Int(0..=4294967295)"), found("Int(0..=4294967295)")]);
}
