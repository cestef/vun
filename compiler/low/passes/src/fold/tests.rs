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
