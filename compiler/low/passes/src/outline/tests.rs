use vun_index::Idx;
use vun_inline::{Block, Bound, FormId, Inst, OpId, Operand, RegRef, SizeId};
use vun_isa_x86_64::Reg;
use vun_lir::testing::origin;
use vun_lir::{
    AbiType, Alias, Builder, Callee, Conv, Ext, InstKind, Linkage, Output, Scalar, Signature, Target,
    Terminator, Unit,
};
use vun_origin::OriginTable;

use super::Outline;

type Extra = Vec<Operand<vun_lir::Value, Output, vun_lir::Address>>;

/// A function reading a constant and writing a slot in a block of assembly, with `more` blocks
/// after it, and `extra` the operands the block has besides those two.
fn unit(conv: Conv, more: bool, extra: Extra) -> (Unit, OriginTable) {
    unit_of(conv, more, vec![extra])
}

/// The same with a block of assembly for each list of operands in `blocks`.
fn unit_of(conv: Conv, more: bool, blocks: Vec<Extra>) -> (Unit, OriginTable) {
    let origins = OriginTable::new();
    let o = origin(&origins);
    let signature = Signature { conv, params: Vec::new(), result: None, variadic: None, convention: None };
    let mut b = Builder::new(Linkage::Internal, signature, o);
    let slot = b.slot(8, 8, o);
    let place = Output { addr: b.slot_addr(slot, o), ty: Scalar::I64 };
    let value = b.constant(Scalar::I64, 7, o);
    for extra in blocks {
        let mut operands = vec![
            Operand::Bound(Bound::In { value, reg: None }),
            Operand::Bound(Bound::Out { place, reg: None }),
        ];
        operands.extend(extra);
        let inst = Inst { op: OpId(0), form: FormId(0), size: SizeId(3), operands, origin: o };
        let arch = vun_items::lang::arch_x86_64::arith.parent().expect("a namespace");
        let block = Block { arch, insts: vec![inst], returns: true, needs: Vec::new() };
        b.push(InstKind::Asm(Box::new(block), None), None, o);
    }
    if more {
        let next = b.block(&[], o);
        b.terminate(Terminator::Jump(Target { block: next, args: Vec::new() }));
        b.switch_to(next);
    }
    b.terminate(Terminator::Return(None));
    let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
    unit.functions.push(b.finish());
    (unit, origins)
}

#[test]
fn a_block_moves_into_a_function_taking_what_it_reads_and_the_address_of_what_it_writes() {
    let (mut unit, origins) = unit(Conv::Vun, true, Vec::new());
    assert_eq!(unit.outline(), 1);
    unit.verify(&origins).expect("well formed");
    let thunk = &unit.functions[vun_lir::FuncId::new(1)];
    assert_eq!(thunk.signature.params, [AbiType::Int(Scalar::I64, Ext::Zero), AbiType::Address(Alias::Any)]);
    assert_eq!(thunk.linkage, Linkage::Internal);
    assert!(thunk.insts.iter().any(|(_, i)| matches!(i.kind, InstKind::Asm(..))));
    let caller = &unit.functions[vun_lir::FuncId::new(0)];
    assert!(!caller.insts.iter().any(|(_, i)| matches!(i.kind, InstKind::Asm(..))));
    let calls: Vec<_> = caller
        .insts
        .iter()
        .filter_map(|(_, i)| match &i.kind {
            InstKind::Call { callee: Callee::Func(f), args } => Some((f.index(), args.len())),
            _ => None,
        })
        .collect();
    assert_eq!(calls, [(1, 2)]);
}

#[test]
fn a_block_naming_the_stack_pointer_a_naked_function_and_a_function_of_nothing_else_stay() {
    let reg = |number| Operand::Reg(RegRef { class: vun_inline::ClassId(0), number });
    let (mut with_stack, _) = unit(Conv::Vun, true, vec![reg(Reg::Rsp as u8)]);
    assert_eq!(with_stack.outline(), 0);
    let (mut with_scratch, _) = unit(Conv::Vun, true, vec![reg(Reg::Rdi as u8)]);
    assert_eq!(with_scratch.outline(), 1);
    let (mut bare, _) = unit(Conv::Naked, true, Vec::new());
    assert_eq!(bare.outline(), 0);
    let (mut alone, _) = unit(Conv::Vun, false, Vec::new());
    assert_eq!(alone.outline(), 0);
    assert_eq!(alone.functions.len(), 1);
}

#[test]
fn blocks_naming_one_scratch_register_stay_together() {
    let reg = |r: Reg| Operand::Reg(RegRef { class: vun_inline::ClassId(0), number: r as u8 });
    let (mut shared, _) = unit_of(Conv::Vun, true, vec![vec![reg(Reg::Rdi)], vec![reg(Reg::Rdi)]]);
    assert_eq!(shared.outline(), 0);
    let (mut apart, _) = unit_of(Conv::Vun, true, vec![vec![reg(Reg::Rdi)], vec![reg(Reg::Rsi)]]);
    assert_eq!(apart.outline(), 2);
}
