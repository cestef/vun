//! The encoder against the bytes the architecture manual gives.

use crate::x86_64::encode::{Alu, Asm};
use crate::x86_64::{Mem, Reg, Rm, Size};

fn encoded(f: impl FnOnce(&mut Asm)) -> Vec<u8> {
    let mut asm = Asm::default();
    f(&mut asm);
    asm.code
}

#[test]
fn register_operands_take_rex_where_extended_or_bytes() {
    assert_eq!(encoded(|a| a.mov_rr(Reg::Rax, Reg::R9)), [0x4c, 0x89, 0xc8]);
    assert_eq!(encoded(|a| a.alu(Alu::Add, Size::B32, Reg::Rcx, Reg::Rdx)), [0x01, 0xd1]);
    assert_eq!(encoded(|a| a.alu(Alu::Cmp, Size::B8, Reg::Rsi, Reg::Rax)), [0x40, 0x38, 0xc6]);
    assert_eq!(encoded(|a| a.zero_extend(Size::B8, Reg::Rax, Reg::Rdi)), [0x40, 0x0f, 0xb6, 0xc7]);
    assert_eq!(encoded(|a| a.alu_imm(Alu::Sub, Size::B64, Rm::Reg(Reg::Rsp), 16)), [0x48, 0x83, 0xec, 0x10]);
}

#[test]
fn memory_operands_take_sib_and_displacements_where_the_base_needs_them() {
    assert_eq!(encoded(|a| a.load(Size::B64, Reg::Rax, Mem::at(Reg::Rbp, -8))), [0x48, 0x8b, 0x45, 0xf8]);
    assert_eq!(encoded(|a| a.store(Size::B32, Mem::at(Reg::Rsp, 0), Reg::Rcx)), [0x89, 0x0c, 0x24]);
    assert_eq!(encoded(|a| a.load(Size::B8, Reg::Rdx, Mem::at(Reg::R13, 0))), [0x41, 0x0f, 0xb6, 0x55, 0x00]);
    assert_eq!(
        encoded(|a| a.lea(Reg::R11, Mem::at(Reg::Rbp, -512))),
        [0x4c, 0x8d, 0x9d, 0x00, 0xfe, 0xff, 0xff]
    );
}

#[test]
fn immediates_take_the_shortest_move() {
    assert_eq!(encoded(|a| a.mov_ri(Reg::Rax, 1)), [0xb8, 1, 0, 0, 0]);
    assert_eq!(encoded(|a| a.mov_ri(Reg::R8, u64::MAX)), [0x49, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff]);
    assert_eq!(encoded(|a| a.mov_ri(Reg::Rcx, 1 << 40)), [0x48, 0xb9, 0, 0, 0, 0, 0, 1, 0, 0]);
}
