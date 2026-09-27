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
    let table = session.db.get::<ArchTable>(&(package, Arch::X86_64)).unwrap();
    let read: Vec<_> = table.ops.iter().map(|op| (op.name.as_str().to_owned(), op.forms.len())).collect();
    let generated: Vec<_> =
        vun_asm_x86_64::Op::ALL.iter().map(|op| (op.mnemonic().to_owned(), op.forms().len())).collect();
    assert_eq!(read, generated);
}
