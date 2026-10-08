use crate::Fold;
use vun_index::Idx;
use vun_origin::OriginTable;

use vun_lir::func::{Conv, Linkage, Signature};
use vun_lir::testing::origin;
use vun_lir::{Builder, CastOp, Cond, InstKind, Scalar};

#[test]
fn a_function_folds_what_became_constant_and_branches_on_it() {
    use vun_lir::{Block, Target, Terminator};
    let origins = OriginTable::new();
    let at = origin(&origins);
    let signature = Signature {
        conv: Conv::Vun,
        params: Vec::new(),
        result: Some(vun_lir::AbiType::Int(Scalar::I32, vun_lir::Ext::Sign)),
        variadic: None,
        convention: None,
    };
    let mut build = Builder::new(Linkage::Internal, signature, at);
    let [minus_two, zero] = [0xfe, 0].map(|bits| build.constant(Scalar::I8, bits, at));
    // Pushed as they are, as a pass leaves them once their operands are constants.
    let wide = build.push(
        InstKind::Cast { op: CastOp::SExt, value: minus_two, to: Scalar::I32 },
        Some(Scalar::I32),
        at,
    );
    let wide = wide.unwrap();
    let less = build
        .push(InstKind::Compare { cond: Cond::SLt, a: minus_two, b: zero }, Some(Scalar::I8), at)
        .unwrap();
    let (yes, no) = (build.block(&[], at), build.block(&[], at));
    let to = |block| Target { block, args: Vec::new() };
    build.terminate(Terminator::Branch { cond: less, then: to(yes), otherwise: to(no) });
    for block in [yes, no] {
        build.switch_to(block);
        build.terminate(Terminator::Return(Some(wide)));
    }
    let mut func = build.finish();
    assert_eq!(func.fold(), 1, "the branch; the constants are folded in place");
    assert_eq!(func.constant(wide), Some((Scalar::I32, 0xffff_fffe)));
    assert_eq!(func.blocks[Block::new(0)].term, Terminator::Jump(to(yes)));
}

#[test]
fn an_operation_in_an_unreached_block_reads_what_another_such_block_folded_away() {
    use vun_lir::{BinOp, Terminator};
    let origins = OriginTable::new();
    let at = origin(&origins);
    let signature = Signature {
        conv: Conv::Vun,
        params: Vec::new(),
        result: Some(vun_lir::AbiType::Int(Scalar::I32, vun_lir::Ext::Sign)),
        variadic: None,
        convention: None,
    };
    let mut build = Builder::new(Linkage::Internal, signature, at);
    let zero = build.constant(Scalar::I32, 0, at);
    build.terminate(Terminator::Return(Some(zero)));
    // Blocks nothing enters, as code after a `break` is: the one reading comes first.
    let reading = build.block(&[], at);
    let defining = build.block(&[Scalar::I32], at);
    build.switch_to(defining);
    let param = build.params(defining)[0];
    let same =
        build.push(InstKind::Binary { op: BinOp::Add, a: param, b: zero }, Some(Scalar::I32), at).unwrap();
    build.terminate(Terminator::Return(Some(same)));
    build.switch_to(reading);
    let twice =
        build.push(InstKind::Binary { op: BinOp::Add, a: same, b: same }, Some(Scalar::I32), at).unwrap();
    build.terminate(Terminator::Return(Some(twice)));
    let mut func = build.finish();
    assert_eq!(func.fold(), 1, "the addition of zero");
}

#[test]
fn the_truth_of_a_comparison_compared_with_zero_is_the_negated_comparison() {
    use vun_lir::{AbiType, Ext, Terminator};
    let origins = OriginTable::new();
    let at = origin(&origins);
    let signature = Signature {
        conv: Conv::Vun,
        params: vec![AbiType::Int(Scalar::I32, Ext::Sign)],
        result: Some(AbiType::Int(Scalar::I8, Ext::Zero)),
        variadic: None,
        convention: None,
    };
    let mut build = Builder::new(Linkage::Internal, signature, at);
    let entry = build.current();
    let x = build.params(entry)[0];
    let ten = build.constant(Scalar::I32, 10, at);
    let [zero, one] = [0, 1].map(|bits| build.constant(Scalar::I8, bits, at));
    let less = build.push(InstKind::Compare { cond: Cond::SLt, a: x, b: ten }, Some(Scalar::I8), at).unwrap();
    let not_less =
        build.push(InstKind::Compare { cond: Cond::Eq, a: less, b: zero }, Some(Scalar::I8), at).unwrap();
    let still_less =
        build.push(InstKind::Compare { cond: Cond::Eq, a: less, b: one }, Some(Scalar::I8), at).unwrap();
    let both = build
        .push(InstKind::Binary { op: vun_lir::BinOp::Xor, a: not_less, b: still_less }, Some(Scalar::I8), at)
        .unwrap();
    build.terminate(Terminator::Return(Some(both)));
    let mut func = build.finish();
    func.fold();
    let conds: Vec<Cond> = func
        .blocks
        .iter()
        .flat_map(|(_, b)| &b.insts)
        .filter_map(|&i| match func.insts[i].kind {
            InstKind::Compare { cond, .. } => Some(cond),
            _ => None,
        })
        .collect();
    // `(x < 10) == 0` is `x >= 10`; `(x < 10) == 1` is the comparison itself, which stays used.
    assert_eq!(conds, [Cond::SLt, Cond::SGe]);
}
