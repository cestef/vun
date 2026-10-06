//! Reflection turns types into data (spec 03, section 6).

use crate::common::Fixture;
use vun_build::layout::MANIFEST;
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
        found("u32", "Option(u32)"),
        found("u32", "Option(bool)"),
        found("u8", "&[u8]"),
        found("bool", "u8"),
    ];
    assert_eq!(fixture.reported(&file), reported);
}

#[test]
fn a_package_collects_its_members_marked_by_an_attribute() {
    let fixture = Fixture::new("reflection-collect");
    fixture.root.write(
        MANIFEST,
        "\
package := .{ .name = \"app\", .id = 1, .version = \"0.1.0\", .edition = 2026, .include = .{ \"build.vun\", \"app.vun\", \"app\" } };
build := (b: &mut std.build.Build) {};
",
    );
    fixture.root.write("app/more.vun", "#[app.Mark]\nthird := 4;\n");
    let file = fixture.root.write(
        "app.vun",
        "\
Mark := struct {};
#[Mark]
first := 1;
second := 2;
#[Mark]
_hidden := 3;
/// Members first, then the namespaces below, each in order; `_hidden` is not visible to `std`,
/// whose `collect` looks.
counted := () { $assert(std.meta.collect(app, Mark).len() == 2); };
nested := () { $assert(std.meta.collect(app, Mark)[1].decl.name.len() == 5); };
/// A collected member's value, read through the namespace declaring it.
read := () { $assert(std.meta.collect(app, Mark)[1].ns.$(std.meta.collect(app, Mark)[1].decl.name) == 4); };
",
    );
    assert_eq!(fixture.reported(&file), []);
}

#[test]
fn the_declarations_of_a_struct_and_the_parts_of_a_function_type_are_data() {
    let fixture = Fixture::new("reflection-declarations");
    let file = fixture.root.write(
        "interface.vun",
        "\
Shape := interface(($Self: type) -> type { struct {
    area: (self: &Self, scale: u8) -> u64 = _;
    name := (self: &Self) -> bool { true };
} });
Table: type = Shape(u8);
/// Declarations in order, a requirement apart from what has a value.
declared := () {
    $assert(std.meta.members(Table).len() == 2);
    $assert(std.meta.members(Table)[0].required);
    $assert(!std.meta.members(Table)[1].required);
};
/// A requirement's type is the function type with `Self` bound.
typed := () {
    $if .some(f) = std.meta.members(Table)[0].type {
        $if .some(s) = std.meta.signature(f) {
            $assert(lang.meta.items(s.Params).len() == 2);
            $assert(std.meta.same(type, lang.meta.items(s.Params)[0], &u8));
            $assert(std.meta.same(type, lang.meta.items(s.Params)[1], u8));
            $assert(std.meta.same(type, s.Returns, u64));
        } else { $assert(false); }
    } else { $assert(false); }
};
/// A type that is no function has no signature, and tuples are made of their items.
parts := () {
    $assert(std.meta.signature(u8) == .none);
    $assert(lang.meta.items(std.meta.Tuple(lang.meta.items((u8, bool)))).len() == 2);
    $assert(std.meta.same(type, lang.meta.items(std.meta.Tuple(&[u8, bool]))[1], bool));
};
",
    );
    assert_eq!(fixture.reported(&file), []);
}
