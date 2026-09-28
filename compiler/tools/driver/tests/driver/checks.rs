//! Checks of unsafe code at run time (spec 07, section 7): under the program's checks, an
//! operation marked `#[lang.check.by]` runs as its checked version, a trap site of its own.

use vun_diag::{DiagId, errors};
use vun_intern::Symbol;
use vun_sema::target::Choice;
use vun_target::Check;

use crate::common::Fixture;

#[test]
fn a_raw_read_runs_as_its_checked_version_under_the_pointer_checks() {
    let fixture = Fixture::new("checks-pointers");
    let path = fixture.root.write(
        "p.vun",
        "\
#[link.export(\"first\")]
first := (xs: &[4]u8) -> u8 { unsafe lang.mem.pointer_to([4]u8, xs).cast(u8).read() };
",
    );
    let lowered = |checks: &[Check]| {
        let mut session = fixture.session();
        session.add_checks(checks.iter().copied());
        let file = session.load(&path).unwrap();
        let unit = session.lir(file).as_ref().clone().unwrap();
        assert_eq!(unit.verify(session.origins), Ok(()));
        let imports: Vec<Symbol> = unit.imports.iter().map(|(_, i)| i.symbol).collect();
        (unit.sites.len(), imports)
    };
    let (sites, imports) = lowered(&[]);
    let (checked, checked_imports) = lowered(&[Check::Pointers]);
    assert_eq!(checked, sites + 1);
    for symbol in ["vun_check_access", "vun_check_fail"] {
        assert!(!imports.contains(&Symbol::intern(symbol)));
        assert!(checked_imports.contains(&Symbol::intern(symbol)), "{checked_imports:?}");
    }
}

/// A program reading an item of an array through a raw pointer.
const THIRD: &str = "\
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {
    xs: [4]u8 = [1, 2, 3, 4];
    third := unsafe lang.mem.pointer_to([4]u8, &xs).cast(u8).offset(2).read();
    std.os.Exit.of((third + 7).to(Int(0..=125)))
};
";

/// The same, reading past the array's end: into its slot's redzone.
const PAST: &str = "\
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {
    xs: [4]u8 = [1, 2, 3, 4];
    past := unsafe lang.mem.pointer_to([4]u8, &xs).cast(u8).offset(5).read();
    std.os.Exit.of((past % 100).to(Int(0..=125)))
};
";

/// The statuses `source`, built with `checks`, ends with through each backend.
fn checked_statuses(name: &str, source: &str, checks: &[Check]) -> Vec<Option<i32>> {
    let fixture = Fixture::new(name);
    let path = fixture.root.write("main.vun", source);
    let mut session = fixture.session();
    session.add_checks(checks.iter().copied());
    let file = session.load(&path).unwrap();
    let runs = session.run_each(file).unwrap_or_else(|_| panic!("{:#?}", session.diag));
    runs.iter().map(|r| r.output.status.code()).collect()
}

#[test]
fn a_checked_program_runs_through_each_backend_over_its_runtime_shadow() {
    let statuses = checked_statuses("checks-run", THIRD, &[Check::Pointers]);
    assert!(statuses.len() > 1 && statuses.iter().all(|&s| s == Some(10)), "{statuses:?}");
}

#[test]
fn a_read_past_a_slot_traps_the_same_way_through_each_backend() {
    let statuses = checked_statuses("checks-past", PAST, &[Check::Pointers]);
    let trapped = Some(101);
    assert!(statuses.len() > 1 && statuses.iter().all(|&s| s == trapped), "{statuses:?}");
}

#[test]
fn a_runtime_without_the_operations_of_the_checks_is_refused() {
    let fixture = Fixture::new("checks-unsupported");
    let path = fixture.root.write("main.vun", THIRD);
    let mut session = fixture.session();
    session.choice = Choice::Path(vec![Symbol::intern("wasi"), Symbol::intern("wasm32")]);
    session.add_checks([Check::Pointers]);
    let file = session.load(&path).unwrap();
    let _ = session.compile(file);
    let ids: Vec<DiagId> = session.diag.sorted(session.origins).into_iter().map(|d| d.id.clone()).collect();
    assert_eq!(ids, [errors::CHECK_UNSUPPORTED]);
}

/// Memory of a fixed allocator given back, then `then`.
fn freed(then: &str) -> String {
    format!(
        "\
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {{
    mut buffer: [64]u8 = [0; 64];
    mut fixed := std.mem.alloc.Fixed.init(&mut buffer);
    p := fixed.alloc(8, 8).expect(\"fits\");
    unsafe p.write(5);
    fixed.free(p, 8, 8);
    {then}
}};
"
    )
}

#[test]
fn memory_given_back_twice_or_read_once_given_back_traps_through_each_backend() {
    let twice = freed("fixed.free(p, 8, 8);\n    std.os.Exit.of(3)");
    let after = freed("std.os.Exit.of(unsafe p.read().to(Int(0..=125)))");
    let trapped = Some(101);
    let runs = [
        checked_statuses("checks-twice", &twice, &[Check::Allocations]),
        checked_statuses("checks-after", &after, &[Check::Pointers, Check::Allocations]),
    ];
    for statuses in runs {
        assert!(statuses.len() > 1 && statuses.iter().all(|&s| s == trapped), "{statuses:?}");
    }
    // Without the pointer checks, the read goes through.
    assert!(checked_statuses("checks-unread", &after, &[Check::Allocations]).iter().all(|&s| s == Some(5)));
}

#[test]
fn an_assembly_block_under_the_asm_checks_carries_its_site_and_the_runtime_helpers() {
    let fixture = Fixture::new("checks-asm");
    let path = fixture.root.write(
        "main.vun",
        "\
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {
    mut r: u64 = 0;
    unsafe lang.arch.x86_64.asm {
        mov {out r}, 7
    };
    std.os.Exit.of((r % 100).to(Int(0..=125)))
};
",
    );
    let checks = |kinds: &[Check]| {
        let mut session = fixture.session();
        session.choice = Choice::Path(vec![Symbol::intern("linux"), Symbol::intern("x86_64")]);
        session.add_checks(kinds.iter().copied());
        let file = session.load(&path).unwrap();
        let unit = session.lir(file).as_ref().clone().unwrap();
        assert_eq!(unit.verify(session.origins), Ok(()));
        let insts = unit.functions.iter().flat_map(|(_, f)| f.insts.iter().map(|(_, i)| i.kind.clone()));
        let checks: Vec<Option<vun_lir::AsmCheck>> = insts
            .filter_map(|kind| match kind {
                vun_lir::InstKind::Asm(_, check) => Some(check),
                _ => None,
            })
            .collect();
        (checks, unit.sites.len())
    };
    let (unchecked, _) = checks(&[]);
    assert!(!unchecked.is_empty() && unchecked.iter().all(Option::is_none));
    let (checked, sites) = checks(&[Check::Asm]);
    let exported = |a: vun_lir::Address| matches!(a, vun_lir::Address::Func(_));
    assert!(!checked.is_empty());
    for check in &checked {
        let check = check.expect("each block is checked");
        assert!((check.site as usize) < sites && exported(check.write) && exported(check.stack));
    }
}

/// How a program built for a target ran here.
enum Ran {
    /// With this status, or none where a signal ended it.
    Exited(Option<i32>),
    /// Not at all: this machine runs no program of the target.
    Elsewhere,
}

/// How `source`, built for `target` by `backend` with the asm checks, runs here.
fn asm_run(name: &str, target: [&str; 2], backend: &str, source: &str) -> Ran {
    let fixture = Fixture::new(name);
    let path = fixture.root.write("main.vun", source);
    let mut session = fixture.session();
    session.choice = Choice::Path(target.iter().map(|&n| Symbol::intern(n)).collect());
    session.codegen.backend = Some(Symbol::intern(backend));
    session.add_checks([Check::Asm]);
    let file = session.load(&path).unwrap();
    // Choosing the target equips it with the machine's tools, and a runner where it needs one.
    let _ = session.lir(file);
    let native = cfg!(target_arch = "x86_64") && target == ["linux", "x86_64"];
    if !cfg!(target_os = "linux") || !native && session.runner.is_empty() {
        return Ran::Elsewhere;
    }
    let program = session.build(file, None).unwrap_or_else(|_| panic!("{:#?}", session.diag));
    // A runner the toolchain names but this machine lacks runs nothing either.
    match session.command(&program).status() {
        Ok(status) => Ran::Exited(status.code()),
        Err(why) if why.kind() == std::io::ErrorKind::NotFound && !session.runner.is_empty() => {
            Ran::Elsewhere
        }
        Err(why) => panic!("{why}"),
    }
}

/// A program writing 7 through a block of `arch`, `write` with `past` for its offset, `past`
/// bytes into its local, exiting with what the local holds.
fn writing(arch: &str, write: &str, past: u32) -> String {
    let write = write.replace("{past}", &past.to_string());
    format!(
        "\
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {{
    mut x: u64 = 0;
    n: u64 = 7;
    p := unsafe lang.mem.pointer_to(u64, &x);
    unsafe lang.arch.{arch}.asm {{
        {write}
    }};
    std.os.Exit.of((x % 100).to(Int(0..=125)))
}};
"
    )
}

/// A program whose block of `arch` moves the stack pointer, `moves`, and returns so.
fn moving(arch: &str, moves: &str) -> String {
    format!(
        "\
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {{
    unsafe lang.arch.{arch}.asm {{
        {moves}
    }};
    .ok
}};
"
    )
}

/// Each architecture's block writing through a binding, and moving the stack pointer.
const ARCHES: [([&str; 2], &str, &str, &str); 2] = [
    (["linux", "x86_64"], "x86_64", "mov [{in p} + {past}], {in n}", "push rax"),
    (["linux", "arm64"], "arm64", "str {in n}, [{in p}, {past}]", "sub sp, sp, 16"),
];

#[test]
fn a_block_writing_past_its_memory_traps_under_the_asm_checks() {
    for (target, arch, write, _) in ARCHES {
        let run = |past| {
            asm_run(&format!("checks-asm-{arch}-{past}"), target, "native", &writing(arch, write, past))
        };
        let (Ran::Exited(within), Ran::Exited(past)) = (run(0), run(8)) else { continue };
        assert_eq!((within, past), (Some(7), Some(101)), "{arch}");
    }
}

#[test]
fn a_block_returning_with_the_stack_moved_traps_through_native_code_and_c() {
    for (target, arch, _, moves) in ARCHES {
        for backend in ["native", "c"] {
            let name = format!("checks-asm-stack-{arch}-{backend}");
            let Ran::Exited(status) = asm_run(&name, target, backend, &moving(arch, moves)) else { continue };
            assert_eq!(status, Some(101), "{arch} {backend}");
        }
    }
}
