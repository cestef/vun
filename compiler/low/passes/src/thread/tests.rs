use crate::{Promote, Sweep, Thread};
use vun_index::Idx;
use vun_intern::Symbol;
use vun_origin::OriginTable;

use vun_lir::func::{Conv, Linkage, Signature};
use vun_lir::testing::origin;
use vun_lir::{AbiType, Block, Builder, Cond, Ext, FuncId, Scalar, Target, Terminator, Unit};

/// `c ? 3 : 4` as lowering builds it: a join receiving `true` or `false`, then testing it.
fn joined() -> (Unit, OriginTable) {
    let origins = OriginTable::new();
    let at = origin(&origins);
    let int = AbiType::Int(Scalar::I8, Ext::Zero);
    let signature = Signature { conv: Conv::C, params: vec![int.clone()], result: Some(int) };
    let mut build = Builder::new(Linkage::Export(Symbol::intern("f")), signature, at);
    let c = build.params(build.current())[0];
    let [yes, no, join, three, four] = [0; 5].map(|_| build.block(&[], at));
    let join_param = build.block(&[Scalar::I8], at);
    let _ = join;
    let to = |block, args| Target { block, args };
    build.terminate(Terminator::Branch { cond: c, then: to(yes, vec![]), otherwise: to(no, vec![]) });
    for (block, bit) in [(yes, 1), (no, 0)] {
        build.switch_to(block);
        let k = build.constant(Scalar::I8, bit, at);
        build.terminate(Terminator::Jump(to(join_param, vec![k])));
    }
    build.switch_to(join_param);
    let p = build.params(join_param)[0];
    let one = build.constant(Scalar::I8, 1, at);
    let holds = build.compare(Cond::Eq, p, one, at);
    build.terminate(Terminator::Branch { cond: holds, then: to(three, vec![]), otherwise: to(four, vec![]) });
    for (block, n) in [(three, 3), (four, 4)] {
        build.switch_to(block);
        let v = build.constant(Scalar::I8, n, at);
        build.terminate(Terminator::Return(Some(v)));
    }
    let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
    unit.functions.push(build.finish());
    (unit, origins)
}

#[test]
fn a_join_deciding_on_the_constant_passed_to_it_is_jumped_over() {
    let (mut unit, origins) = joined();
    let f = FuncId::new(0);
    assert_eq!(unit.functions[f].thread(64), 4, "two threaded, then two forwarded");
    assert_eq!(unit.verify(&origins), Ok(()));
    // The entry branches straight to the blocks returning 3 and 4.
    let entry = &unit.functions[f].blocks[Block::new(0)].term;
    let targets: Vec<usize> = entry.targets().map(|t| t.block.index()).collect();
    assert_eq!(targets, [4, 5]);
    assert_eq!(unit.functions[f].thread(64), 0, "nothing is left");
}

#[test]
fn a_join_whose_parameter_is_read_after_it_is_jumped_over_through_a_slot() {
    let (mut unit, origins) = joined();
    let f = FuncId::new(0);
    // The block returning 3 returns the join's parameter instead.
    let func = &mut unit.functions[f];
    let p = func.blocks[Block::new(6)].params[0];
    func.blocks[Block::new(4)].term = Terminator::Return(Some(p));
    assert_eq!(unit.verify(&origins), Ok(()));
    assert!(unit.functions[f].thread(64) > 0);
    assert_eq!(unit.verify(&origins), Ok(()));
    let func = &mut unit.functions[f];
    assert_eq!(func.promote(64), 1, "the parameter, demoted");
    func.sweep();
    assert_eq!(unit.verify(&origins), Ok(()));
    // Each block returning returns a constant: 1, passed where the join was, or 4.
    let func = &unit.functions[f];
    let mut returned: Vec<_> = func
        .blocks
        .iter()
        .filter_map(|(_, b)| match b.term {
            Terminator::Return(Some(v)) => func.constant(v),
            _ => None,
        })
        .collect();
    returned.sort_unstable();
    assert_eq!(returned, [(Scalar::I8, 1), (Scalar::I8, 4)]);
}

#[test]
fn a_block_whose_constant_is_read_after_it_is_not_jumped_over() {
    let origins = OriginTable::new();
    let at = origin(&origins);
    let int = AbiType::Int(Scalar::I8, Ext::Zero);
    let signature = Signature { conv: Conv::C, params: vec![int.clone()], result: Some(int) };
    let mut build = Builder::new(Linkage::Export(Symbol::intern("f")), signature, at);
    let c = build.params(build.current())[0];
    let [other, holding, after] = [0; 3].map(|_| build.block(&[], at));
    let to = |block| Target { block, args: vec![] };
    build.terminate(Terminator::Branch { cond: c, then: to(holding), otherwise: to(other) });
    build.switch_to(other);
    build.terminate(Terminator::Jump(to(holding)));
    build.switch_to(holding);
    let k = build.constant(Scalar::I8, 7, at);
    build.terminate(Terminator::Jump(to(after)));
    build.switch_to(after);
    build.terminate(Terminator::Return(Some(k)));
    let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
    unit.functions.push(build.finish());
    unit.functions[FuncId::new(0)].thread(64);
    assert_eq!(unit.verify(&origins), Ok(()));
}
