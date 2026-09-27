//! Inline assembly: each instruction encoded by its form over the registers its bindings are
//! given, outputs stored through their addresses, and shapes a backend cannot encode refused.

use vun_asm_x86_64::{Base, Mem, Op, Operand, Size};
use vun_backend::{AsmOperand, Missing};
use vun_inline::{Block, Bound, Disp, FormId, Inst, OpId, SizeId};
use vun_lir::{Access, Asm, Builder, InstKind, Output, Scalar, Terminator, Value};
use vun_origin::OriginId;

use super::{compile, compiled};
use crate::x86_64::X86;

/// The index of `op` in its table, as the block names it.
fn op(op: Op) -> OpId {
    OpId(u16::try_from(Op::ALL.iter().position(|&o| o == op).expect("an instruction")).expect("few"))
}

fn size(size: Size) -> SizeId {
    SizeId(u8::try_from(Size::ALL.iter().position(|&s| s == size).expect("a width")).expect("few"))
}

/// `lea {out r}, [{in n} + disp]` over `index` if any, into a slot; returns `r`.
fn lea(b: &mut Builder, n: Value, origin: OriginId, index: bool) {
    let slot = b.slot(8, 8, origin);
    let addr = b.push(InstKind::SlotAddr(slot), Some(Scalar::Ptr), origin).expect("an address");
    let base = || vun_inline::Base::Bound(Bound::In { value: n, reg: None });
    let address = vun_inline::Mem { base: Some(base()), index: index.then(|| (base(), 2)), disp: Disp::Bytes(3) };
    let operands = vec![
        vun_inline::Operand::Bound(Bound::Out { place: Output { addr, ty: Scalar::I64 }, reg: None }),
        vun_inline::Operand::Mem(address),
    ];
    let inst = Inst { op: op(Op::Lea), form: FormId(0), size: size(Size::B64), operands, origin };
    let asm: Asm = Block { arch: vun_items::lang::arch::x86_64, insts: vec![inst], returns: true };
    b.push(InstKind::Asm(Box::new(asm)), None, origin);
    let r = b.load(Scalar::I64, addr, Access::Plain, origin);
    b.terminate(Terminator::Return(Some(r)));
}

#[test]
fn a_block_computes_in_the_registers_its_bindings_are_given() {
    let code = compiled::<X86>(|b, n, origin| lea(b, n, origin, false));
    let insts = code.code.listing.iter().filter_map(|line| match line {
        vun_asm::Line::Inst { inst, .. } => Some(*inst),
        vun_asm::Line::Label(_) => None,
    });
    let insts: Vec<_> = insts.collect();
    let lea = insts.iter().find(|i| i.op == Op::Lea).expect("the block's instruction");
    let [Operand::Reg(out), Operand::Mem(Mem { base: Base::Reg(_), disp: 3 })] = lea.operands() else {
        panic!("{lea:?} takes a register and an address three past one")
    };
    // The output's register stored through the slot's address once the block ends.
    let stored = insts.iter().any(|i| {
        i.op == Op::Mov && matches!(i.operands(), [Operand::Mem(_), Operand::Reg(r)] if r == out)
    });
    assert!(stored, "{insts:#?}");
}

#[test]
fn memory_x86_64_cannot_address_is_refused() {
    let Err(lack) = compile::<X86>(|b, n, origin| lea(b, n, origin, true)) else {
        panic!("an indexed address compiled")
    };
    assert_eq!(lack.missing, Missing::AsmOperand(AsmOperand::Indexed));
}
