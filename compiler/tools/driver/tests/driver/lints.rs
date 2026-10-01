//! Lints (spec 10, section 4): a package's own lints and the standard ones run on each file it
//! holds, each report at its lint's level, unless `#[lint.allow]` silences it.

use vun_build::layout::MANIFEST;
use vun_diag::{DiagId, Severity};

use crate::common::Fixture;

/// The id and severity of what checking `source`, the root file of a package `app`, reports.
fn linted(name: &str, source: &str) -> Vec<(DiagId, Severity)> {
    let fx = Fixture::new(name);
    let app = Fixture::manifest("app", 1, "0.1.0");
    let dir = fx.write("", &app, source);
    let mut session = fx.session();
    let file = session.load(&Fixture::file(&dir, &app)).unwrap();
    session.check(file);
    session.diag.sorted(session.origins).into_iter().map(|d| (d.id.clone(), d.severity)).collect()
}

const CASE: DiagId = DiagId::new("std.lints.naming.case");

#[test]
fn a_package_lint_checks_each_call_at_its_level() {
    let reported = linted(
        "lints-call",
        "\
#[lint.define]
raw := lang.lint.Lint{
    .group = .suspicious,
    .default = .some(.warn),
    .call = (cx: &mut lang.lint.Cx, call: lang.lint.Call) {
        if call.calls(poke) {
            cx.report(call.origin, \"poke outside a driver\", &[]);
        }
    },
};

poke := (x: u8) -> u8 { x };
twice := (x: u8) -> u8 { poke(poke(x)) };
once := (x: u8) -> u8 { x };
",
    );
    let raw = (DiagId::new("app.raw"), Severity::Warning);
    assert_eq!(reported, [raw.clone(), raw]);
}

#[test]
fn names_against_the_naming_rule_are_denied_unless_allowed() {
    let reported = linted(
        "lints-naming",
        "\
ring_buffer := struct { Len: usize };
WriteAll := (x: u8) -> u8 { x };
Pair := ($T: type) -> type { struct { a: T } };
make_pair := ($T: type) -> type { Pair(T) };
Color := enum { Red, green };
#[lint.allow(std.lints.naming.case, \"mirrors the vendor register names\")]
HAL_UART_Init := 0;
_private_ok := 1;
_PrivateType := struct {};
u24 := unsigned(24);
Dma := struct { start := (self: &mut Dma) {}; };
dma_ops := struct { start := (x: u8) -> u8 { x }; };
timer := struct { start := (self: &timer) {}; };
",
    );
    // `ring_buffer` and `Len`, `WriteAll`, `make_pair`, `Red` and `timer`, a type since its
    // method receives it.
    assert_eq!(reported, vec![(CASE, Severity::Error); 6]);
}

#[test]
fn an_alias_of_a_type_is_annotated_to_bind_one() {
    let reported = linted(
        "lints-alias",
        "\
Index := Int(0..9);
Small: type = Int(0..9);
Nothing: type = ();
unit := ();
Kind := interface(($Self: type) -> type { struct {} });
Callback := (x: u8) -> u8;
",
    );
    // Only `Index`, a name written like a type that nothing says is one.
    assert_eq!(reported, vec![(CASE, Severity::Error)]);
}

#[test]
fn a_build_sets_the_levels_of_lints_and_groups() {
    let fx = Fixture::new("lints-levels");
    fx.root.write(
        MANIFEST,
        "\
package := .{ .name = \"app\", .id = 1, .version = \"0.1.0\", .edition = 2026, .include = .{ \"build.vun\", \"app.vun\" } };
build := (b: &mut std.build.Build) {
    b.lints(.{ .{ std.lints.naming, .warn }, .{ std.lints.style.unused, .deny }, .{ std.lints.style, .allow } });
};
",
    );
    let path = fx.root.write("app.vun", "WriteAll := () -> u8 { x := 1; 0 };\n");
    let mut session = fx.session();
    let file = session.load(&path).unwrap();
    session.check(file);
    let reported: Vec<_> =
        session.diag.sorted(session.origins).into_iter().map(|d| (d.id.clone(), d.severity)).collect();
    // The group `naming` lowered to `warn`; `unused` denied, the pair naming it winning over its
    // group's.
    assert_eq!(
        reported,
        [(CASE, Severity::Warning), (DiagId::new("std.lints.style.unused"), Severity::Error)]
    );
}

#[test]
fn the_fix_allowing_a_lint_puts_its_attribute_on_the_line_above_at_the_indent() {
    let fx = Fixture::new("lints-allow-fix");
    let app = Fixture::manifest("app", 1, "0.1.0");
    let source = "holder := struct {\n    Wrong := 1;\n};\n";
    let dir = fx.write("", &app, source);
    let mut session = fx.session();
    let file = session.load(&Fixture::file(&dir, &app)).unwrap();
    session.check(file);
    let reported = session.diag.sorted(session.origins);
    let allow = reported
        .iter()
        .filter(|d| d.id == CASE)
        .flat_map(|d| d.fixes.iter())
        .find(|f| f.msg.to_string().starts_with("allow it here"))
        .expect("a fix allowing the lint");
    let [edit] = allow.edits.as_slice() else { panic!("{allow:#?}") };
    assert!(edit.text.starts_with("#[lang.lint.allow(std.lints.naming.case"), "{}", edit.text);
    assert!(edit.text.ends_with("\n    "), "{:?}", edit.text);
}
