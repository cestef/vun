//! Inline assembly: each instruction encoded by its form over the registers its bindings are
//! given, outputs stored through their addresses, and shapes a backend cannot encode refused.

use vun_backend::{AsmOperand, Missing};
use vun_inline::{Block, Bound, Disp, FormId, Inst, OpId, SizeId};
use vun_items::Item;
use vun_lir::{Access, Builder, InstKind, Output, Scalar, Terminator, Value};
use vun_origin::OriginId;

use super::{compile, compiled};
use crate::arm64::Arm;
use crate::x86_64::X86;

type Operand = vun_inline::Operand<Value, Output, vun_lir::Address>;

/// The index of `x` in `all`, as a block names entries of a table.
fn index<T: PartialEq>(all: &[T], x: &T) -> usize {
    all.iter().position(|y| y == x).expect("an entry of the table")
}

/// A block of `arch` of one instruction, the `op`th at the `size`th width by its `form`th form,
/// writing a slot then returned.
fn block(
    b: &mut Builder,
    origin: OriginId,
    arch: Item,
    [op, form, size]: [usize; 3],
    operands: Vec<Operand>,
) {
    let slot = b.slot(8, 8, origin);
    let addr = b.push(InstKind::SlotAddr(slot), Some(Scalar::Ptr), origin).expect("an address");
    let out = Operand::Bound(Bound::Out { place: Output { addr, ty: Scalar::I64 }, reg: None });
    let (op, form) = (OpId(u16::try_from(op).expect("few")), FormId(u16::try_from(form).expect("few")));
    let size = SizeId(u8::try_from(size).expect("few"));
    let inst = Inst { op, form, size, operands: [out].into_iter().chain(operands).collect(), origin };
    b.push(InstKind::Asm(Box::new(Block { arch, insts: vec![inst], returns: true }), None), None, origin);
    let r = b.load(Scalar::I64, addr, Access::Plain, origin);
    b.terminate(Terminator::Return(Some(r)));
}

/// The instructions a listing holds.
fn insts<I: Copy>(listing: &[vun_asm::Line<I>]) -> Vec<I> {
    let insts = listing.iter().filter_map(|line| match line {
        vun_asm::Line::Inst { inst, .. } => Some(*inst),
        vun_asm::Line::Label(_) => None,
    });
    insts.collect()
}

/// `lea {out r}, [{in n} + 3]`, the address indexed by `n` too where `indexed`.
fn lea(b: &mut Builder, n: Value, origin: OriginId, indexed: bool) {
    use vun_asm_x86_64::{Op, Size};
    let base = || vun_inline::Base::Bound(Bound::In { value: n, reg: None });
    let address =
        vun_inline::Mem { base: Some(base()), index: indexed.then(|| (base(), 2)), disp: Disp::Bytes(3) };
    let entry = [index(Op::ALL, &Op::Lea), 0, index(&Size::ALL, &Size::B64)];
    block(b, origin, vun_items::lang::arch::x86_64, entry, vec![Operand::Mem(address)]);
}

#[test]
fn a_block_computes_in_the_registers_its_bindings_are_given() {
    use vun_asm_x86_64::{Base, Mem, Op, Operand};
    let insts = insts(&compiled::<X86>(|b, n, origin| lea(b, n, origin, false)).code.listing);
    // The block's instruction: a register, and an address three past another.
    let out = insts.iter().find_map(|i| match (i.op, i.operands()) {
        (Op::Lea, [Operand::Reg(out), Operand::Mem(Mem { base: Base::Reg(_), disp: 3 })]) => Some(*out),
        _ => None,
    });
    let out = out.unwrap_or_else(|| panic!("{insts:#?}"));
    // The output's register stored through the slot's address once the block ends.
    let stored = |i: &vun_asm_x86_64::Inst<_>| {
        i.op == Op::Mov && matches!(i.operands(), [Operand::Mem(_), Operand::Reg(r)] if *r == out)
    };
    assert!(insts.iter().any(stored), "{insts:#?}");
}

#[test]
fn memory_x86_64_cannot_address_is_refused() {
    let Err(lack) = compile::<X86>(|b, n, origin| lea(b, n, origin, true)) else {
        panic!("an indexed address compiled")
    };
    assert_eq!(lack.missing, Missing::AsmOperand(AsmOperand::Indexed));
}

#[test]
fn arm64_encodes_a_block_by_the_form_it_was_checked_against() {
    use vun_asm_arm64::{Kind, Op, Operand, Size};
    // `add {out r}, {in n}, 3`, by the form adding an immediate.
    let form = Op::Add.forms().iter().position(|f| f.args.iter().any(|a| matches!(a.kind, Kind::Imm(_))));
    let entry =
        [index(Op::ALL, &Op::Add), form.expect("a form adding an immediate"), index(&Size::ALL, &Size::B64)];
    let code = compiled::<Arm>(|b, n, origin| {
        let operands =
            vec![vun_inline::Operand::Bound(Bound::In { value: n, reg: None }), vun_inline::Operand::Imm(3)];
        block(b, origin, vun_items::lang::arch::arm64, entry, operands);
    });
    let insts = insts(&code.code.listing);
    let out = insts.iter().find_map(|i| match (i.op, i.operands()) {
        (Op::Add, [Operand::Reg(out), Operand::Reg(_), Operand::Imm(3)]) => Some(*out),
        _ => None,
    });
    let out = out.unwrap_or_else(|| panic!("{insts:#?}"));
    let stored = |i: &vun_asm_arm64::Inst<_>| {
        i.op == Op::Str && matches!(i.operands(), [Operand::Reg(r), Operand::Mem(_)] if *r == out)
    };
    assert!(insts.iter().any(stored), "{insts:#?}");
}
