//! Parts of `std` used from programs: formatting through `std.fmt.print`, whose format string is
//! split at compile time, and the tuples, string literals and slices it is built on.

use crate::common::Fixture;
use vun_diag::errors;
use vun_intern::Symbol;
use vun_sema::target::Choice;

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

#[test]
fn traps_in_a_callees_signature_are_reported_once() {
    let fixture = Fixture::new("std-signature-reports");
    let file = fixture.root.write(
        "signature.vun",
        "\
Digit := ($n: u8) -> type {
    $if n > 9 { $std.diag.error(.{ .msg = \"one digit at most\" }); }
    u8
};
at := ($n: u8) -> Digit(n) { n };
wide := () -> u8 { at(12) };
fine := () -> u8 { at(7) };
Short := ($n: u8) -> type { [n - 10]u8 };
short := ($n: u8) -> Short(n) { [0; n - 10] };
three := () { x := short(3); };
div := ($n: u8) -> [10 / n]u8 { [0; 10 / n] };
zero := () { x := div(0); };
Seven := Short(3);
seven: Seven = [];
",
    );
    // Each trap once: `-7` is no length of `lang.mem.Array`, and `seven` reads a declaration
    // whose own check reports it.
    let lines: Vec<_> = fixture.reported(&file);
    let ids: Vec<_> = lines.iter().map(|(id, _)| id.clone()).collect();
    assert_eq!(
        ids,
        [errors::COMPTIME_REPORTED, errors::COMPTIME_TRAP, errors::COMPTIME_TRAP, errors::COMPTIME_TRAP],
        "{lines:?}"
    );
}

#[test]
fn a_fixed_buffer_is_an_allocator() {
    let fixture = Fixture::new("std-fixed");
    let file = fixture.root.write(
        "fixed.vun",
        "\
..std.mem.alloc.{ Fixed };
run := () -> u8 {
    mut storage: [16]u8 = [0; 16];
    mut fixed := Fixed.init(&mut storage);
    first := fixed.alloc(4, 4);
    x: u8 = first;
    x
};
",
    );
    // `alloc` is found through the impl of `Allocator` and has its type.
    let found = fixture.reported(&file);
    assert_eq!(found.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(), [errors::TYPES_MISMATCH]);
    assert!(found[0].1.contains("found `Result("), "{found:?}");
}

#[test]
fn a_hosted_program_reads_its_arguments() {
    let fixture = Fixture::new("std-args");
    let file = fixture.root.write(
        "args.vun",
        "\
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {
    mut out := sys.stdout();
    std.fmt.print(&mut out, \"{} args\\n\", .{ sys.args().len() });
    for arg in sys.args() {
        std.fmt.print(&mut out, \"{}\\n\", .{ arg });
        wrong: u8 = arg;
    }
    .ok
};
",
    );
    // Each argument is the bytes the system passed.
    let found = fixture.reported(&file);
    assert_eq!(found.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(), [errors::TYPES_MISMATCH]);
    assert!(found[0].1.ends_with("found `&[Int(0..=255)]`"), "{found:?}");
}

#[test]
fn print_runs_at_compile_time_through_any_writer() {
    let fixture = Fixture::new("std-print-comptime");
    let file = fixture.root.write(
        "counted.vun",
        "\
Count := struct { n: usize };
#[impl(std.fmt.Writer(Count))]
CountWriter := struct {
    Error := ();
    closed: () = ();
    write := (self: &mut Count, bytes: &[u8]) -> Result(usize, ()) {
        self.n = (self.n + bytes.len()).sat(usize);
        .ok(bytes.len())
    };
};
counted := ($A: type, args: A) -> usize {
    mut c := Count{ .n = 0 };
    std.fmt.print(&mut c, \"<{} {}>\", args);
    c.n
};
numbers := () -> usize { xs: [3]u16 = [1, 20, 300]; counted(.{ xs, \"ab\" }) };
negative := () -> usize { none: [0]u16 = []; counted(.{ -45, none }) };
nothing := () -> usize { mut c := Count{ .n = 0 }; std.fmt.print(&mut c, \"plain\", .{}); c.n };
seventeen: Int(numbers()..=numbers()) = 17;
eight: Int(negative()..=negative()) = 8;
five: Int(nothing()..=nothing()) = 5;
wrong: Int(nothing()..=nothing()) = 4;
",
    );
    // `<[1, 20, 300] ab>`, `<-45 []>` and `plain`: arrays of other items than bytes are written
    // item by item, through the defaults of `Writer` that the impl does not declare.
    assert_eq!(fixture.check(&file), [errors::INT_DOES_NOT_FIT]);
}

#[test]
fn the_c_library_row_is_the_chosen_systems() {
    let rows: [(&[&str], &str); 2] =
        [(&["linux", "gnu", "x86_64"], "__errno_location"), (&["macos", "arm64"], "__error")];
    for (target, errno) in rows {
        let fixture = Fixture::new(&format!("std-libc-{}", target[0]));
        let file = fixture.root.write(
            "p.vun",
            "\
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {
    mut out := sys.stdout();
    std.fmt.print(&mut out, \"{}\\n\", .{ 1 });
    .ok
};
",
        );
        let mut session = fixture.session();
        session.choice = Choice::Path(target.iter().map(|&n| Symbol::intern(n)).collect());
        let file = session.load(&file).unwrap();
        let lowered = session.lir(file);
        let unit = lowered.as_ref().as_ref().unwrap_or_else(|_| panic!("{target:?}: {:#?}", session.diag));
        // `errno` is reached through the function the system's row names.
        assert!(unit.imports.iter().any(|(_, i)| i.symbol == Symbol::intern(errno)), "{target:?}");
    }
}
