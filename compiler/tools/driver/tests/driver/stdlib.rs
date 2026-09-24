//! Parts of `std` used from programs: formatting through `std.fmt.print`, whose format string is
//! split at compile time, and the tuples, string literals and slices it is built on.

use crate::common::Fixture;
use vun_diag::errors;

#[test]
fn tuples_strings_and_slices_are_typed_and_evaluated() {
    let fixture = Fixture::new("std-parts");
    let file = fixture.root.write(
        "parts.vun",
        "\
pair := () -> u8 { t := .{ 1, 2 }; t.0 + t.$(1) };
past := () -> u8 { t := .{ 1 }; t.1 };
text := () -> &[u8] { \"ab\" };
wanted := (s: &static [2]u8) -> u8 { s[0] };
given := (s: &[2]u8) -> u8 { wanted(s) };
tail := ($s: &[u8]) -> usize { mut n: usize = 1; s[n..].len() };
counted := () { $for i in 0..tail(\"abc\") { x: bool = i; } };
",
    );
    let ids: Vec<_> = fixture.reported(&file).into_iter().map(|(id, _)| id).collect();
    // `t.1` has no item; a plain borrow is no `&static` one; `tail` evaluates to 2, whose steps
    // each report their binder.
    assert_eq!(
        ids,
        [errors::TYPES_NO_MEMBER, errors::TYPES_MISMATCH, errors::TYPES_MISMATCH, errors::TYPES_MISMATCH]
    );
}

#[test]
fn print_writes_its_arguments_by_their_format() {
    let fixture = Fixture::new("std-print");
    let file = fixture.root.write(
        "print.vun",
        "\
P := struct { x: u8 };
main := (sys: &mut std.os.System) -> std.os.Exit {
    mut out := sys.stdout();
    n: u32 = 42;
    std.fmt.print(&mut out, \"{} and {} of {}\\n\", .{ n, -7, \"bytes\" });
    std.fmt.print(&mut out, \"{}\\n\", .{ P{ .x = 1 } });
    .ok
};
",
    );
    let ids: Vec<_> = fixture.reported(&file).into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, [errors::TYPES_NO_MEMBER], "`P` has no `Format` impl");
}

#[test]
fn library_reports_are_shown_at_the_call_that_led_to_them() {
    let fixture = Fixture::new("std-reports");
    let file = fixture.root.write(
        "reports.vun",
        "\
checked := ($n: u8) -> u8 {
    $if n > 9 { $std.diag.error(.{ .msg = \"one digit at most\" }); }
    n
};
digit := () -> u8 { checked(12) };
fine := () -> u8 { checked(7) };
main := (sys: &mut std.os.System) -> std.os.Exit {
    mut out := sys.stdout();
    std.fmt.print(&mut out, \"{} {}\\n\", .{ 1 });
    std.fmt.print(&mut out, \"{}\\n\", .{ 1 });
    .ok
};
",
    );
    let lines: Vec<_> = fixture.reported(&file);
    let ids: Vec<_> = lines.iter().map(|(id, _)| id.clone()).collect();
    assert_eq!(ids, [errors::COMPTIME_REPORTED, errors::COMPTIME_REPORTED], "{lines:?}");
}
