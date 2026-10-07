//! Lints (spec 10, section 4): a package's own lints and the standard ones run on each file it
//! holds, each report at its lint's level, unless `#[lint.allow]` silences it.

use vun_build::layout::MANIFEST;
use vun_diag::{DiagId, Severity, errors};
use vun_sema_lint::{Level, LintLevels};

use crate::common::Fixture;

/// The id and severity of what checking `source`, the root file of a package `app`, reports.
fn linted(name: &str, source: &str) -> Vec<(DiagId, Severity)> {
    linted_at(name, source, &[])
}

/// The same, with the lint levels `levels` given on the command line.
fn linted_at(name: &str, source: &str, levels: &[(&str, Level)]) -> Vec<(DiagId, Severity)> {
    let fx = Fixture::new(name);
    let app = Fixture::manifest("app", 1, "0.1.0");
    let dir = fx.write("", &app, source);
    let mut session = fx.session();
    let file = session.load(&Fixture::file(&dir, &app)).unwrap();
    let package = session.file_ref(file).package;
    let pairs = levels.iter().map(|(id, level)| (DiagId::parse(id), *level));
    session.add_lint_levels(LintLevels::of(pairs).scoped_to([package]));
    session.check(file);
    session.diag.sorted(session.origins).into_iter().map(|d| (d.id, d.severity)).collect()
}

fn case() -> DiagId {
    DiagId::parse("std.lints.naming.case")
}

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
    let raw = (DiagId::parse("app.raw"), Severity::Warning);
    assert_eq!(reported, [raw, raw]);
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
    assert_eq!(reported, vec![(case(), Severity::Error); 6]);
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
    assert_eq!(reported, vec![(case(), Severity::Error)]);
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
        session.diag.sorted(session.origins).into_iter().map(|d| (d.id, d.severity)).collect();
    // The group `naming` lowered to `warn`; `unused` denied, the pair naming it winning over its
    // group's.
    assert_eq!(
        reported,
        [(case(), Severity::Warning), (DiagId::parse("std.lints.style.unused"), Severity::Error)]
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
        .filter(|d| d.id == case())
        .flat_map(|d| d.fixes.iter())
        .find(|f| f.msg.to_string().starts_with("allow it here"))
        .expect("a fix allowing the lint");
    let [edit] = allow.edits.as_slice() else { panic!("{allow:#?}") };
    assert!(edit.text.starts_with("#[lang.lint.allow(std.lints.naming.case"), "{}", edit.text);
    assert!(edit.text.ends_with("\n    "), "{:?}", edit.text);
}

#[test]
fn a_local_allow_silences_the_lint_naming_it() {
    let reported = linted(
        "lints-local-allow",
        "\
keep := () -> u8 {
    #[lang.lint.allow(std.lints.style.unused, \"read through the debugger\")]
    x := 1;
    y := 2;
    0
};
",
    );
    let unused = (DiagId::parse("std.lints.style.unused"), Severity::Warning);
    assert_eq!(reported, [unused]);
}

#[test]
fn the_command_line_sets_the_level_of_a_lint() {
    let reported = linted_at(
        "lints-cli-level",
        "WriteAll := () -> u8 { 0 };\n",
        &[("std.lints.naming.case", Level::Warn)],
    );
    assert_eq!(reported, [(case(), Severity::Warning)]);
}

#[test]
fn an_expected_lint_that_does_not_report_warns() {
    let reported = linted_at(
        "lints-expect-silent",
        "f := () -> u8 { 0 };\n",
        &[("std.lints.style.unused", Level::Expect)],
    );
    assert_eq!(reported, [(errors::LINT_UNFULFILLED, Severity::Warning)]);
}

#[test]
fn an_expected_lint_that_reports_does_not_also_warn() {
    let reported = linted_at(
        "lints-expect-fired",
        "f := () -> u8 { x := 1; 0 };\n",
        &[("std.lints.style.unused", Level::Expect)],
    );
    assert_eq!(reported, [(DiagId::parse("std.lints.style.unused"), Severity::Warning)]);
}

#[test]
fn a_dependency_s_lint_reports_at_most_warn() {
    let fx = Fixture::new("lints-dep-cap");
    let lib = Fixture::manifest("lintlib", 2, "0.1.0");
    fx.write(
        "lintlib",
        &lib,
        "\
#[lint.define]
no_magic := lang.lint.Lint{
    .group = .restriction,
    .default = .some(.deny),
    .decl = (cx: &mut lang.lint.Cx, d: lang.lint.Decl) {
        cx.report(d.origin, \"no magic here\", &[]);
    },
};
",
    );
    let mut app = Fixture::manifest("app", 1, "0.1.0");
    app.deps.push(Fixture::local("lintlib", "../lintlib"));
    let dir = fx.write("app", &app, "answer := 42;\n");
    let mut session = fx.session();
    let file = session.load(&Fixture::file(&dir, &app)).unwrap();
    session.check(file);
    let reported: Vec<_> =
        session.diag.sorted(session.origins).into_iter().map(|d| (d.id, d.severity)).collect();
    assert_eq!(reported, [(DiagId::parse("lintlib.no_magic"), Severity::Warning)]);
}

#[test]
fn a_mutable_local_typed_as_one_integer_is_reported() {
    let pinned = DiagId::parse("std.lints.suspicious.pinned");
    let reported = linted("lints-pinned", "f := () -> u32 { mut n := 0; n };\n");
    assert_eq!(reported, [(pinned, Severity::Warning)]);
    let held = linted("lints-pinned-held", "f := () -> u32 { mut n: u32 = 0; n += 1; n };\n");
    assert_eq!(held, []);
    let fixed = linted("lints-pinned-immutable", "f := () -> u32 { n := 0; n };\n");
    assert_eq!(fixed, []);
}
