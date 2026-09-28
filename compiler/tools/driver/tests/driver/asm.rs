//! Blocks of instructions (spec 07, section 6): sema reads an architecture's table from its
//! declarations in the order the generated assembler numbers it, so the instruction and form a
//! block names are the ones the backend encodes.

use vun_driver::Session;
use vun_harness::{ACCEPT, repo_root};
use vun_sema::asm::{Arch, ArchTable};

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
finish := () -> never { unsafe std.os.libc.exit(7) };
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
