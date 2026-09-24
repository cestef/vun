//! Reflection turns types into data (spec 03, section 6).

use crate::common::Fixture;
use vun_diag::errors;

#[test]
fn fields_generate_members() {
    let fixture = Fixture::new("reflection-fields");
    let file = fixture.root.write(
        "partial.vun",
        "\
Partial := ($T: type) -> type { struct {
    $for f in std.meta.fields(T) { $(f.name): ?f.type = .none, }
} };
Config := struct { baud: u32, parity: bool };
baud := (p: Partial(Config)) -> u32 { p.baud };
parity := (p: Partial(Config)) -> u32 { p.parity };
name := () -> u8 { std.meta.name_of(Config) };
Named := struct { $(std.meta.name_of(Config)): u8 };
named := (n: Named) -> bool { n.Config };
",
    );
    let found =
        |expected: &str, ty: &str| (errors::TYPES_MISMATCH, format!("expected `{expected}`, found `{ty}`"));
    let reported = [
        found("u32", "Option(Int(0..2^32))"),
        found("u32", "Option(bool)"),
        found("u8", "&[Int(0..=255)]"),
        found("bool", "Int(0..=255)"),
    ];
    assert_eq!(fixture.reported(&file), reported);
}
