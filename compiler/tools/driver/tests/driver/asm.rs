//! Blocks of instructions (spec 07, section 6): sema reads an architecture's table from its
//! declarations in the order the generated assembler numbers it, so the instruction and form a
//! block names are the ones the backend encodes.

use vun_driver::Session;
use vun_harness::{ACCEPT, repo_root};
use vun_intern::Symbol;
use vun_lower::Why;
use vun_sema::asm::{Arch, ArchTable};
use vun_sema::target::Choice;

#[test]
fn tables_number_instructions_and_forms_as_the_assemblers_do() {
    let mut session = Session::new();
    let file = session.load(&repo_root().join(ACCEPT).join("capabilities/asm.vun")).unwrap();
    let package = session.file_ref(file).package;
    let read = |arch| {
        let table = session.db.get::<ArchTable>(&(package, arch)).unwrap();
        table.ops.iter().map(|op| (op.name.as_str().to_owned(), op.forms.len())).collect::<Vec<_>>()
    };
    let x86_64: Vec<_> =
        vun_isa_x86_64::Op::ALL.iter().map(|op| (op.mnemonic().to_owned(), op.forms().len())).collect();
    let arm64: Vec<_> =
        vun_isa_arm64::Op::ALL.iter().map(|op| (op.mnemonic().to_owned(), op.forms().len())).collect();
    assert_eq!(read(Arch::X86_64), x86_64);
    let thumb: Vec<_> =
        vun_isa_thumb::Op::ALL.iter().map(|op| (op.mnemonic().to_owned(), op.forms().len())).collect();
    assert_eq!(read(Arch::Arm64), arm64);
    assert_eq!(read(Arch::Thumb), thumb);
}

/// Naked functions, entered as their code, whose blocks reach a function by its symbol: `hop`
/// calls it, `leap` jumps to it, and it ends the program with 7.
const NAKED: &str = "\
#[abi.naked]
hop := () -> never {
    unsafe {
        lang.arch.x86_64.asm {
            sub rsp, 8
            call {finish}
        };
        lang.hint.unreachable()
    }
};
#[abi.naked]
leap := () -> never {
    unsafe lang.arch.x86_64.asm {
        jmp {finish}
    }
};
finish := () -> never { std.os.linux.kernel.end(7) };
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit { if sys.args().len() > 1 { leap() } else { hop() } };
";

#[test]
fn a_naked_function_calls_or_jumps_by_symbol_through_each_backend() {
    if !cfg!(all(target_arch = "x86_64", target_os = "linux")) {
        return;
    }
    for backend in crate::backend::BACKENDS {
        let (_fixture, program) = crate::backend::build("asm-naked", NAKED, backend);
        for (args, way) in [(&[][..], "call"), (&["leap"][..], "jump")] {
            let ran = std::process::Command::new(&program).args(args).output().unwrap();
            assert_eq!(ran.status.code(), Some(7), "{backend} {way}");
        }
    }
}

#[test]
fn floats_are_bound_in_floating_point_registers_through_each_backend() {
    if !cfg!(all(target_arch = "x86_64", target_os = "linux")) {
        return;
    }
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/driver/programs/asm_floats.vun"),
    )
    .unwrap();
    assert_eq!(crate::backend::status_on("asm-floats", &source, &[&["linux", "arm64"]]), Some(0));
}

/// A block writing the register an input is read in, the input used again after it.
const OVERWRITTEN: &str = "\
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {
    x: u64 = (sys.args().len() + 40).wrap(u64);
    mut y: u64 = 0;
    unsafe lang.arch.x86_64.asm {
        lea rcx, [{in(rax) x} + 1]
        mov rax, rcx
        mov {out(rdx) y}, rcx
    };
    std.os.Exit.of(if x == 41 and y == 42 { 7 } else { 1 })
};
";

#[test]
fn a_value_read_in_a_register_a_block_writes_is_kept_apart_through_each_backend() {
    if !cfg!(all(target_arch = "x86_64", target_os = "linux")) {
        return;
    }
    for backend in crate::backend::BACKENDS {
        let (_fixture, program) = crate::backend::build("asm-overwritten", OVERWRITTEN, backend);
        let ran = std::process::Command::new(&program).output().unwrap();
        assert_eq!(ran.status.code(), Some(7), "{backend}");
    }
}

/// A block loading two words exclusively, which Thumb has on Arm's application profile alone.
const PAIRED: &str = "\
#[link.export(\"paired\")]
paired := (at: usize) -> u32 {
    mut (low, high): (u32, u32) = (0, 0);
    unsafe lang.arch.thumb.asm {
        ldrexd {out(r2) low}, {out(r3) high}, [{in(r0) at}]
    };
    low ^ high
};
";

#[test]
fn a_block_is_refused_for_a_processor_lacking_a_feature_one_of_its_forms_needs() {
    let lowered = |target: [&str; 2]| {
        let fixture = crate::common::Fixture::new(&format!("asm-paired-{}", target[1]));
        let path = fixture.root.write("main.vun", PAIRED);
        let mut session = fixture.session();
        session.choice = Choice::Path(target.iter().map(|&n| Symbol::intern(n)).collect());
        let file = session.load(&path).unwrap();
        session.lir(file).as_ref().as_ref().err().map(|unlowered| unlowered.why.clone())
    };
    assert_eq!(lowered(["bare", "cortex_m4f"]), Some(Why::Unavailable(Symbol::intern("application"))));
    assert_eq!(lowered(["linux", "arm"]), None);
}
