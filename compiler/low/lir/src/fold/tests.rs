use vun_index::Idx;
use vun_intern::Symbol;
use vun_origin::OriginTable;

use crate::func::{Conv, Linkage, Signature};
use crate::tests::origin;
use crate::{BinOp, Builder, CastOp, Cond, InstKind, Scalar};

#[test]
fn integer_operations_fold_within_their_width_and_domain() {
    assert_eq!(BinOp::Add.fold(Scalar::I8, 0xff, 1), Some(0));
    assert_eq!(BinOp::Mul.fold(Scalar::I16, 0x100, 0x100), Some(0));
    assert_eq!(BinOp::AShr.fold(Scalar::I8, 0x80, 7), Some(0xff));
    assert_eq!(BinOp::LShr.fold(Scalar::I8, 0x80, 7), Some(1));
    assert_eq!(BinOp::SDiv.fold(Scalar::I32, Scalar::I32.truncate(-7), 2), Some(Scalar::I32.truncate(-3)));
    assert_eq!(BinOp::SRem.fold(Scalar::I32, Scalar::I32.truncate(-7), 2), Some(Scalar::I32.truncate(-1)));
    // Outside the domain the operation is not defined: nothing is folded.
    assert_eq!(BinOp::Shl.fold(Scalar::I8, 1, 8), None);
    assert_eq!(BinOp::UDiv.fold(Scalar::I64, 1, 0), None);
    assert_eq!(BinOp::SDiv.fold(Scalar::I8, 0x80, 0xff), None);
    assert_eq!(BinOp::FAdd.fold(Scalar::F32, 0, 0), None);
    assert_eq!(Cond::SLt.fold(Scalar::I8, 0xff, 0), Some(true));
    assert_eq!(Cond::ULt.fold(Scalar::I8, 0xff, 0), Some(false));
    assert_eq!(CastOp::SExt.fold(Scalar::I8, Scalar::I32, 0x80), Some(0xffff_ff80));
    assert_eq!(CastOp::ZExt.fold(Scalar::I8, Scalar::I32, 0x80), Some(0x80));
    assert_eq!(CastOp::Trunc.fold(Scalar::I32, Scalar::I8, 0x1234), Some(0x34));
    assert_eq!(CastOp::Bitcast.fold(Scalar::I32, Scalar::F32, 0x3f80_0000), Some(0x3f80_0000));
}

#[test]
fn the_builder_folds_constants_and_identities() {
    let origins = OriginTable::new();
    let o = origin(&origins);
    let signature = Signature {
        conv: Conv::Vun,
        params: vec![crate::AbiType::Int(Scalar::I32, crate::Ext::Zero)],
        result: None,
    };
    let mut b = Builder::new(Linkage::Export(Symbol::intern("f")), signature, o);
    let x = b.params(b.current())[0];
    let [six, seven, one] = [6, 7, 1].map(|n| b.constant(Scalar::I32, n, o));
    let product = b.binary(BinOp::Mul, six, seven, o);
    assert_eq!(b.binary(BinOp::Mul, x, one, o), x);
    let less = b.compare(Cond::ULt, product, seven, o);
    assert_eq!(b.select(less, x, product, o), product);
    let func = b.finish();
    let kinds: Vec<&InstKind> = func.insts.iter().map(|(_, i)| &i.kind).collect();
    assert!(kinds.contains(&&InstKind::Const { ty: Scalar::I32, bits: 42 }), "{kinds:?}");
    assert!(!kinds.iter().any(|k| matches!(k, InstKind::Binary { .. } | InstKind::Compare { .. })));
}

#[test]
fn a_function_folds_what_became_constant_and_branches_on_it() {
    use crate::{Block, Target, Terminator};
    let origins = OriginTable::new();
    let at = origin(&origins);
    let signature = Signature {
        conv: Conv::Vun,
        params: Vec::new(),
        result: Some(crate::AbiType::Int(Scalar::I32, crate::Ext::Sign)),
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
