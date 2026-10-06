//! Packs (spec 01, section 4): a function's last parameter `..args: P` gathers the arguments after
//! the others into a tuple, which a C function takes as its variadic arguments (spec 07,
//! section 5).

use vun_driver::compare::Ran;
use vun_harness::{ACCEPT, repo_root};
use vun_intern::Symbol;
use vun_items::{Item, targets};
use vun_lir::ImportKind;
use vun_sema::target::Choice;

use crate::backend;
use crate::common::Fixture;

/// A pack written, inferred, empty, and through a function value: `6 + 36 + 0 + 6`.
const PACKS: &str = "\
sum := (first: u8, ..rest: (u8, u8)) -> u16 { first + rest.0 + rest.1 };
total := ($P: type, ..xs: P) -> u32 {
    mut t: u32 = 0;
    $for i in 0..lang.meta.items(P).len() { t +|= xs.$(i); }
    t
};

start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {
    x: u8 = 5;
    f: (u8, ..(u8, u8)) -> u16 = sum;
    std.os.Exit.of((sum(1, 2, 3) + total(1, x, 30) + total() + f(1, 2, 3)).to(Int(0..=125)))
};
";

#[test]
fn a_pack_gathers_the_arguments_after_the_others() {
    assert_eq!(backend::status_across("packs", PACKS), Some(48));
}

/// `snprintf(buffer, 16, "%d %.1f %lld", 42, 2.5, -7)`: 9 bytes, `42 2.5 -7`, and 100 more
/// where they are those.
const FORMAT: &str = "\
#[link.symbol(\"snprintf\"), abi.c]
format: unsafe (to: *u8, bytes: usize, format: *u8, ..args: .{ i32, f64, i64 }) -> i32 = _;

start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {
    mut spec: [13]u8 = ['%', 'd', ' ', '%', '.', '1', 'f', ' ', '%', 'l', 'l', 'd', 0];
    mut b: [16]u8 = [0; 16];
    n := unsafe format(
        lang.mem.pointer_to([16]u8, &b).cast(u8),
        16,
        lang.mem.pointer_to([13]u8, &spec).cast(u8),
        42,
        2.5,
        -7,
    );
    same := b[0] == '4' and b[1] == '2' and b[3] == '2' and b[4] == '.' and b[5] == '5' and b[7] == '-'
        and b[8] == '7' and b[9] == 0;
    std.os.Exit.of((n + if same { 100 } else { 0 }).to(Int(0..=125)))
};
";

/// The same call with the pack inferred per call from the arguments' types.
const INFERRED: &str = "\
#[link.symbol(\"snprintf\"), abi.c]
format: unsafe ($P: type, to: *u8, bytes: usize, format: *u8, ..args: P) -> i32 = _;

start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {
    mut spec: [13]u8 = ['%', 'd', ' ', '%', '.', '1', 'f', ' ', '%', 'l', 'l', 'd', 0];
    mut b: [16]u8 = [0; 16];
    int: i32 = 42;
    float: f64 = 2.5;
    long: i64 = -7;
    n := unsafe format(
        lang.mem.pointer_to([16]u8, &b).cast(u8),
        16,
        lang.mem.pointer_to([13]u8, &spec).cast(u8),
        int,
        float,
        long,
    );
    same := b[0] == '4' and b[1] == '2' and b[3] == '2' and b[4] == '.' and b[5] == '5' and b[7] == '-'
        and b[8] == '7' and b[9] == 0;
    std.os.Exit.of((n + if same { 100 } else { 0 }).to(Int(0..=125)))
};
";

/// Builds and runs `source` for `target` through each backend, where the machine runs its
/// programs: the exit status, the same through each.
fn status(name: &str, source: &str, target: Item) -> Option<i32> {
    let fixture = Fixture::new(name);
    let path = fixture.root.write("main.vun", source);
    let mut session = fixture.session();
    session.choice = Choice::of(target);
    let file = session.load(&path).unwrap();
    let runs = session.run_each(file, 1).unwrap_or_else(|_| panic!("{:#?}", session.diag));
    assert!(Ran::agree(&runs), "the backends disagree");
    runs.first().and_then(|r| r.output.status.code())
}

#[test]
fn a_variadic_c_call_passes_integers_and_floats_as_c_does() {
    if !cfg!(all(target_arch = "x86_64", target_os = "linux")) {
        return;
    }
    assert_eq!(status("variadic-format", FORMAT, targets::linux_x86_64::gnu), Some(109));
}

#[test]
fn a_variadic_c_call_infers_its_pack_from_the_arguments() {
    if !cfg!(all(target_arch = "x86_64", target_os = "linux")) {
        return;
    }
    assert_eq!(status("variadic-inferred", INFERRED, targets::linux_x86_64::gnu), Some(109));
}

#[test]
fn open_takes_its_permissions_as_a_variadic_argument() {
    let text = std::fs::read_to_string(repo_root().join(ACCEPT).join("programs/files.vun")).unwrap();
    let fixture = Fixture::new("variadic-open");
    let path = fixture.root.write("main.vun", &text);
    let mut session = fixture.session();
    session.choice = Choice::of(targets::macos_arm64::darwin);
    let file = session.load(&path).unwrap();
    let lowered = session.lir(file);
    let unit = lowered.as_ref().as_ref().unwrap_or_else(|_| panic!("{:#?}", session.diag));
    let open = unit.imports.iter().find(|(_, i)| i.symbol == Symbol::intern("open")).map(|(_, i)| &i.kind);
    let Some(ImportKind::Func(signature)) = open else { panic!("no import of `open`") };
    assert_eq!(signature.variadic, Some(2));
}
