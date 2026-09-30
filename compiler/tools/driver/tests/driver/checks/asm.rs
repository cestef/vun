//! The checks of assembly blocks: the memory they write and the stack pointer they return with.

use vun_intern::Symbol;
use vun_items::{Item, targets};
use vun_sema::target::Choice;
use vun_target::Check;

use super::TRAPPED;
use crate::common::Fixture;

#[test]
fn an_assembly_block_under_the_asm_checks_carries_its_site_and_the_runtime_helpers() {
    let fixture = Fixture::new("checks-asm");
    let path = fixture.root.write(
        "main.vun",
        "\
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {
    mut r: u64 = 0;
    unsafe lang.arch.asm({ ..lang.arch.x86_64.names; mov(out(r), 7); });
    std.os.Exit.of((r % 100).to(Int(0..=125)))
};
",
    );
    let checks = |kinds: &[Check]| {
        let mut session = fixture.session();
        session.choice = Choice::of(targets::linux_x86_64::kernel);
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
    // The program's block is checked; the runtime's own, on the paths the checks run on or in a
    // naked function without a frame, are not.
    assert!(checked.iter().any(Option::is_some));
    for check in checked.iter().flatten() {
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
fn asm_run(name: &str, target: Item, backend: &str, source: &str) -> Ran {
    let fixture = Fixture::new(name);
    let path = fixture.root.write("main.vun", source);
    let mut session = fixture.session();
    session.choice = Choice::of(target);
    session.codegen.backend = Some(Symbol::intern(backend));
    session.add_checks([Check::Asm]);
    let file = session.load(&path).unwrap();
    // Choosing the target equips it with the machine's tools, and a runner where it needs one.
    let _ = session.lir(file);
    let native = cfg!(target_arch = "x86_64") && target == targets::linux_x86_64::kernel;
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
    unsafe lang.arch.asm({{ ..lang.arch.{arch}.names; {write}; }});
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
    unsafe lang.arch.asm({{ ..lang.arch.{arch}.names; {moves}; }});
    .ok
}};
"
    )
}

/// Each architecture's block writing through a binding, and moving the stack pointer.
const ARCHES: [(Item, &str, &str, &str); 2] = [
    (targets::linux_x86_64::kernel, "x86_64", "mov(mem(in(p), {past}), in(n))", "push(rax)"),
    (targets::linux_arm64::kernel, "arm64", "str(in(n), mem(in(p), {past}))", "sub(sp, sp, 16)"),
];

#[test]
fn a_block_writing_past_its_memory_traps_under_the_asm_checks() {
    for (target, arch, write, _) in ARCHES {
        let run = |past| {
            asm_run(&format!("checks-asm-{arch}-{past}"), target, "native", &writing(arch, write, past))
        };
        let (Ran::Exited(within), Ran::Exited(past)) = (run(0), run(8)) else { continue };
        assert_eq!((within, past), (Some(7), TRAPPED), "{arch}");
    }
}

#[test]
fn a_block_returning_with_the_stack_moved_traps_through_native_code_and_c() {
    for (target, arch, _, moves) in ARCHES {
        for backend in ["native", "c"] {
            let name = format!("checks-asm-stack-{arch}-{backend}");
            let Ran::Exited(status) = asm_run(&name, target, backend, &moving(arch, moves)) else { continue };
            assert_eq!(status, TRAPPED, "{arch} {backend}");
        }
    }
}
