use crate::{Inline, Thread};
use vun_index::Idx;
use vun_intern::Symbol;
use vun_origin::OriginTable;

use vun_lir::analysis::Reached;
use vun_lir::func::{Conv, Linkage, Signature};
use vun_lir::testing::origin;
use vun_lir::{AbiType, BinOp, Builder, Callee, Ext, FuncId, InstKind, Scalar, Target, Terminator, Unit};

/// `inc(x) = x + 1`, internal and lowered as an entry jumping to its body, and an exported
/// `twice(x) = inc(inc(x))`.
fn unit() -> (Unit, OriginTable) {
    let origins = OriginTable::new();
    let at = origin(&origins);
    let int = AbiType::Int(Scalar::I32, Ext::Zero);
    let signature = |conv| Signature {
        conv,
        params: vec![int.clone()],
        result: Some(int.clone()),
        variadic: None,
        convention: None,
    };
    let mut inc = Builder::new(Linkage::Internal, signature(Conv::Vun), at);
    let x = inc.params(inc.current())[0];
    let body = inc.block(&[], at);
    inc.terminate(Terminator::Jump(Target { block: body, args: Vec::new() }));
    inc.switch_to(body);
    let one = inc.constant(Scalar::I32, 1, at);
    let sum = inc.binary(BinOp::Add, x, one, at);
    inc.terminate(Terminator::Return(Some(sum)));
    let mut twice = Builder::new(Linkage::Export(Symbol::intern("twice")), signature(Conv::C), at);
    let x = twice.params(twice.current())[0];
    let call = |b: &mut Builder, arg| {
        b.push(
            InstKind::Call { callee: Callee::Func(FuncId::new(0)), args: vec![arg] },
            Some(Scalar::I32),
            at,
        )
    };
    let first = call(&mut twice, x).unwrap();
    let again = call(&mut twice, first).unwrap();
    twice.terminate(Terminator::Return(Some(again)));
    let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
    unit.functions.push(inc.finish());
    unit.functions.push(twice.finish());
    (unit, origins)
}

#[test]
fn a_small_function_of_two_blocks_is_inlined_and_left_unreached() {
    let (mut unit, origins) = unit();
    assert_eq!(unit.inline(), 2);
    assert_eq!(unit.verify(&origins), Ok(()));
    let twice = &mut unit.functions[FuncId::new(1)];
    // The caller's block, then for each call the entry, the body and what followed the call.
    assert_eq!(twice.blocks.len(), 7);
    assert!(twice.thread(64) > 0);
    assert_eq!(twice.blocks.iter().filter(|(_, b)| b.term != Terminator::Unreachable).count(), 1);
    assert_eq!(unit.verify(&origins), Ok(()));
    let twice = &unit.functions[FuncId::new(1)];
    let calls = twice.insts.iter().filter(|(_, i)| matches!(i.kind, InstKind::Call { .. })).count();
    assert_eq!(calls, 0);
    let reached = Reached::of(&unit);
    assert!(reached.contains(FuncId::new(1)) && !reached.contains(FuncId::new(0)));
}
