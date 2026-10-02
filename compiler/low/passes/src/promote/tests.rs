use crate::Promote;
use vun_index::Idx;
use vun_intern::Symbol;
use vun_origin::{OriginId, OriginTable};

use vun_lir::func::{Conv, Linkage, Signature};
use vun_lir::testing::origin;
use vun_lir::{
    AbiType, Access, BinOp, Block, Builder, Cond, FuncId, InstKind, Scalar, Target, Terminator, Unit,
};

fn unit(b: Builder) -> Unit {
    let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
    unit.functions.push(b.finish());
    unit
}

fn jump(block: Block) -> Terminator {
    Terminator::Jump(Target { block, args: Vec::new() })
}

/// `sum(n)` as lowering gives it: the counter and the total are slots.
fn sum_in_slots(origin: OriginId) -> Unit {
    let int = AbiType::Int(Scalar::I64, vun_lir::Ext::Zero);
    let signature = Signature {
        conv: Conv::C,
        params: vec![int.clone()],
        result: Some(int),
        variadic: None,
        convention: None,
    };
    let mut b = Builder::new(Linkage::Export(Symbol::intern("sum")), signature, origin);
    let n = b.params(b.current())[0];
    let [i, total] = [b.slot(8, 8, origin), b.slot(8, 8, origin)];
    let zero = b.constant(Scalar::I64, 0, origin);
    for slot in [i, total] {
        let at = b.slot_addr(slot, origin);
        b.store(zero, at, Access::Plain, origin);
    }
    let (head, body, exit) = (b.block(&[], origin), b.block(&[], origin), b.block(&[], origin));
    b.terminate(jump(head));
    b.switch_to(head);
    let at = b.slot_addr(i, origin);
    let now = b.load(Scalar::I64, at, Access::Plain, origin);
    let more = b.compare(Cond::ULt, now, n, origin);
    b.terminate(Terminator::Branch {
        cond: more,
        then: Target { block: body, args: Vec::new() },
        otherwise: Target { block: exit, args: Vec::new() },
    });
    b.switch_to(body);
    let (i_at, total_at) = (b.slot_addr(i, origin), b.slot_addr(total, origin));
    let now = b.load(Scalar::I64, i_at, Access::Plain, origin);
    let sum = b.load(Scalar::I64, total_at, Access::Plain, origin);
    let added = b.binary(BinOp::Add, sum, now, origin);
    b.store(added, total_at, Access::Plain, origin);
    let one = b.constant(Scalar::I64, 1, origin);
    let next = b.binary(BinOp::Add, now, one, origin);
    b.store(next, i_at, Access::Plain, origin);
    b.terminate(jump(head));
    b.switch_to(exit);
    let at = b.slot_addr(total, origin);
    let result = b.load(Scalar::I64, at, Access::Plain, origin);
    b.terminate(Terminator::Return(Some(result)));
    unit(b)
}

fn memory(unit: &Unit) -> usize {
    let func = &unit.functions[FuncId::new(0)];
    let kinds = func.blocks.iter().flat_map(|(_, b)| &b.insts).map(|&i| &func.insts[i].kind);
    kinds
        .filter(|k| matches!(k, InstKind::Load { .. } | InstKind::Store { .. } | InstKind::SlotAddr(_)))
        .count()
}

#[test]
fn locals_in_slots_become_the_parameters_of_the_loop() {
    let origins = OriginTable::new();
    let mut unit = sum_in_slots(origin(&origins));
    assert_eq!(unit.verify(&origins), Ok(()));
    assert_eq!(unit.functions[FuncId::new(0)].promote(64), 2);
    assert_eq!(unit.verify(&origins), Ok(()));
    let func = &unit.functions[FuncId::new(0)];
    assert!(func.slots.is_empty());
    assert_eq!(memory(&unit), 0);
    // The loop's head receives both; the body and the exit have one predecessor each.
    let params: Vec<usize> = func.blocks.iter().map(|(_, b)| b.params.len()).collect();
    assert_eq!(params, [1, 2, 0, 0]);
}

#[test]
fn a_slot_whose_address_escapes_stays() {
    let origins = OriginTable::new();
    let o = origin(&origins);
    let signature =
        Signature { conv: Conv::Vun, params: Vec::new(), result: None, variadic: None, convention: None };
    let mut b = Builder::new(Linkage::Internal, signature, o);
    let [kept, promoted] = [b.slot(8, 8, o), b.slot(4, 4, o)];
    let at = b.slot_addr(kept, o);
    let four = b.constant(Scalar::I64, 4, o);
    let past = b.offset(at, four, o);
    let word = b.constant(Scalar::I32, 7, o);
    b.store(word, past, Access::Plain, o);
    let at = b.slot_addr(promoted, o);
    b.store(word, at, Access::Plain, o);
    b.terminate(Terminator::Return(None));
    let mut unit = unit(b);
    assert_eq!(unit.functions[FuncId::new(0)].promote(64), 1);
    assert_eq!(unit.verify(&origins), Ok(()));
    assert_eq!(unit.functions[FuncId::new(0)].slots.len(), 1);
}

#[test]
fn a_slot_read_before_any_store_reads_zero_and_one_read_in_parts_stays() {
    let origins = OriginTable::new();
    let o = origin(&origins);
    let signature = Signature {
        conv: Conv::C,
        params: Vec::new(),
        result: Some(AbiType::Scalar(Scalar::F64)),
        variadic: None,
        convention: None,
    };
    let mut b = Builder::new(Linkage::Internal, signature, o);
    let [unset, halves] = [b.slot(8, 8, o), b.slot(8, 8, o)];
    let at = b.slot_addr(halves, o);
    let half = b.load(Scalar::I32, at, Access::Plain, o);
    let wide = b.cast(vun_lir::CastOp::ZExt, half, Scalar::I64, o);
    b.store(wide, at, Access::Plain, o);
    let at = b.slot_addr(unset, o);
    let read = b.load(Scalar::F64, at, Access::Plain, o);
    b.terminate(Terminator::Return(Some(read)));
    let mut unit = unit(b);
    assert_eq!(unit.functions[FuncId::new(0)].promote(64), 1);
    assert_eq!(unit.verify(&origins), Ok(()));
    let func = &unit.functions[FuncId::new(0)];
    let first = func.blocks[Block::new(0)].insts[0];
    assert_eq!(func.insts[first].kind, InstKind::Const { ty: Scalar::F64, bits: 0 });
    assert_eq!(func.slots.len(), 1);
}
