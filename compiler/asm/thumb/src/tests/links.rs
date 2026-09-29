//! Branches to labels, patched when the function ends, and to symbols, left to the linker.

use vun_asm::RelocKind;
use vun_isa_thumb::{Cc, Op, Operand, Reg, Size};

use super::Sym;
use crate::{Asm, Emit, Processor};

#[test]
fn a_branch_to_a_label_is_patched_where_it_is_bound() {
    let mut asm = Asm::<Sym>::new(Processor::every());
    let (back, ahead) = (asm.label(), asm.label());
    asm.bind(back);
    asm.emit(Op::B, Size::B32, &[Operand::Label(ahead)]);
    asm.emit(Op::Bcond, Size::B32, &[Operand::Cond(Cc::Ne), Operand::Label(back)]);
    asm.bind(ahead);
    let code = asm.finish().expect("both bound");
    // `b.w` 4 bytes ahead of `pc`, then `bne.w` back 8 bytes from its own `pc`.
    assert_eq!(code.bytes, [0x00, 0xf0, 0x02, 0xb8, 0x7f, 0xf4, 0xfc, 0xaf]);
}

#[test]
fn a_call_and_an_address_are_left_to_the_linker() {
    let mut asm = Asm::<Sym>::new(Processor::every());
    asm.emit(Op::Bl, Size::B32, &[Operand::Sym("f")]);
    asm.emit(Op::Movw, Size::B32, &[Operand::Reg(Reg::R0), Operand::Low("d")]);
    asm.emit(Op::Movt, Size::B32, &[Operand::Reg(Reg::R0), Operand::High("d")]);
    let code = asm.finish().expect("no label");
    let kinds: Vec<(u64, RelocKind, i64)> =
        code.relocs.iter().map(|r| (r.offset, r.kind, r.addend)).collect();
    assert_eq!(
        kinds,
        [(0, RelocKind::ThumbCall, -4), (4, RelocKind::ThumbLow16, 0), (8, RelocKind::ThumbHigh16, 0)]
    );
}
