use vun_index::Idx;
use vun_intern::Symbol;
use vun_origin::{OriginId, OriginTable};

use crate::func::{Conv, Linkage, Signature};
use crate::tests::origin;
use crate::{
    AbiType, BinOp, Builder, Callee, CastOp, Ext, FuncId, Function, InstKind, Scalar, Terminator, Unit,
};

fn signature(conv: Conv, param: Scalar, result: Scalar) -> Signature {
    let int = |ty| AbiType::Int(ty, Ext::Zero);
    Signature { conv, params: vec![int(param)], result: Some(int(result)) }
}

fn call(b: &mut Builder, f: usize, arg: crate::Value, ty: Scalar, at: OriginId) -> crate::Value {
    let kind = InstKind::Call { callee: Callee::Func(FuncId::new(f)), args: vec![arg] };
    b.push(kind, Some(ty), at).unwrap()
}

/// `wrap(x: i128) -> i64`, `step(x: i64) = wrap(x * k + c)` computed in `i128`, both internal,
/// and an exported `run(x) = step(x)`, with a block nothing jumps to.
fn unit(at: OriginId) -> Unit {
    let mut wrap = Builder::new(Linkage::Internal, signature(Conv::Vun, Scalar::I128, Scalar::I64), at);
    let x = wrap.params(wrap.current())[0];
    let mask = wrap.constant(Scalar::I128, u128::from(u64::MAX), at);
    let low = wrap.binary(BinOp::And, x, mask, at);
    let low = wrap.cast(CastOp::Trunc, low, Scalar::I64, at);
    wrap.terminate(Terminator::Return(Some(low)));
    let mut step = Builder::new(Linkage::Internal, signature(Conv::Vun, Scalar::I64, Scalar::I64), at);
    let x = step.params(step.current())[0];
    let wide = step.cast(CastOp::ZExt, x, Scalar::I128, at);
    let k = step.constant(Scalar::I128, 6_364_136_223_846_793_005, at);
    let product = step.binary(BinOp::Mul, wide, k, at);
    let c = step.constant(Scalar::I128, 1_442_695_040_888_963_407, at);
    let sum = step.binary(BinOp::Add, product, c, at);
    let wrapped = call(&mut step, 0, sum, Scalar::I64, at);
    step.terminate(Terminator::Return(Some(wrapped)));
    let signature = signature(Conv::C, Scalar::I64, Scalar::I64);
    let mut run = Builder::new(Linkage::Export(Symbol::intern("run")), signature, at);
    let x = run.params(run.current())[0];
    let stepped = call(&mut run, 1, x, Scalar::I64, at);
    run.terminate(Terminator::Return(Some(stepped)));
    let unreached = run.block(&[], at);
    run.switch_to(unreached);
    run.terminate(Terminator::Return(Some(x)));
    let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
    for function in [wrap.finish(), step.finish(), run.finish()] {
        unit.functions.push(function);
    }
    unit
}

fn kinds(f: &Function) -> Vec<&InstKind> {
    f.blocks.iter().flat_map(|(_, b)| &b.insts).map(|&i| &f.insts[i].kind).collect()
}

#[test]
fn a_callee_inlined_brings_its_callees_inlined_and_a_truncated_product_is_narrow() {
    let origins = OriginTable::new();
    let mut unit = unit(origin(&origins));
    unit.simplify();
    assert_eq!(unit.verify(&origins), Ok(()));
    let run = &unit.functions[FuncId::new(2)];
    let binaries: Vec<BinOp> = kinds(run)
        .into_iter()
        .filter_map(|kind| match kind {
            InstKind::Binary { op, .. } => Some(*op),
            InstKind::Call { .. } => panic!("every call is inlined"),
            _ => None,
        })
        .collect();
    assert_eq!(binaries, [BinOp::Mul, BinOp::Add]);
    assert!(run.values.iter().all(|(_, v)| v.ty == Scalar::I64), "nothing is computed in i128");
}

#[test]
fn an_unreached_block_is_dropped_and_an_unread_value_swept() {
    let origins = OriginTable::new();
    let at = origin(&origins);
    let mut unit = unit(at);
    let run = &mut unit.functions[FuncId::new(2)];
    let entry = run.entry().unwrap();
    let (unread, _) = run.define(InstKind::Const { ty: Scalar::I32, bits: 7 }, Some(Scalar::I32), at);
    run.blocks[entry].insts.insert(0, unread);
    assert_eq!(run.sweep(), 2, "the block and the constant");
    assert_eq!(run.blocks.len(), 1);
    assert_eq!(kinds(run).len(), 1, "the call stays: it acts");
    assert_eq!(unit.verify(&origins), Ok(()));
}

#[test]
fn a_computation_dominated_by_an_equal_one_reuses_its_value() {
    let origins = OriginTable::new();
    let at = origin(&origins);
    let signature = signature(Conv::C, Scalar::I64, Scalar::I64);
    let mut b = Builder::new(Linkage::Export(Symbol::intern("f")), signature, at);
    let x = b.params(b.current())[0];
    let k = b.constant(Scalar::I64, 3, at);
    let first = b.binary(BinOp::Mul, x, k, at);
    let next = b.block(&[], at);
    b.terminate(Terminator::Jump(crate::Target { block: next, args: Vec::new() }));
    b.switch_to(next);
    let k = b.constant(Scalar::I64, 3, at);
    // The same product, its operands the other way round.
    let again = b.binary(BinOp::Mul, k, x, at);
    let sum = b.binary(BinOp::Add, first, again, at);
    b.terminate(Terminator::Return(Some(sum)));
    let mut f = b.finish();
    assert_eq!(f.reuse(), 2, "the constant and the product");
    let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
    unit.functions.push(f);
    assert_eq!(unit.verify(&origins), Ok(()));
}

#[test]
fn a_loop_parameter_only_passed_back_is_swept() {
    let origins = OriginTable::new();
    let at = origin(&origins);
    let signature = signature(Conv::C, Scalar::I64, Scalar::I64);
    let mut b = Builder::new(Linkage::Export(Symbol::intern("f")), signature, at);
    let x = b.params(b.current())[0];
    let head = b.block(&[Scalar::I64, Scalar::I64], at);
    let exit = b.block(&[], at);
    let to = |block, args| crate::Target { block, args };
    b.terminate(Terminator::Jump(to(head, vec![x, x])));
    b.switch_to(head);
    let [used, carried] = [b.params(head)[0], b.params(head)[1]];
    let one = b.constant(Scalar::I64, 1, at);
    let next = b.binary(BinOp::Sub, used, one, at);
    let more = b.compare(crate::Cond::Ne, next, one, at);
    b.terminate(Terminator::Branch {
        cond: more,
        then: to(head, vec![next, carried]),
        otherwise: to(exit, Vec::new()),
    });
    b.switch_to(exit);
    b.terminate(Terminator::Return(Some(next)));
    let mut f = b.finish();
    assert_eq!(f.sweep(), 1);
    assert_eq!(f.blocks.iter().nth(1).unwrap().1.params.len(), 1);
    let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
    unit.functions.push(f);
    assert_eq!(unit.verify(&origins), Ok(()));
}

#[test]
fn a_block_past_an_equality_reads_the_constant_compared() {
    let origins = OriginTable::new();
    let at = origin(&origins);
    let signature = signature(Conv::C, Scalar::I64, Scalar::I64);
    let mut b = Builder::new(Linkage::Export(Symbol::intern("f")), signature, at);
    let x = b.params(b.current())[0];
    let five = b.constant(Scalar::I64, 5, at);
    let equal = b.compare(crate::Cond::Eq, x, five, at);
    let (then, otherwise) = (b.block(&[], at), b.block(&[], at));
    let to = |block| crate::Target { block, args: Vec::new() };
    b.terminate(Terminator::Branch { cond: equal, then: to(then), otherwise: to(otherwise) });
    b.switch_to(then);
    let one = b.constant(Scalar::I64, 1, at);
    let next = b.binary(BinOp::Add, x, one, at);
    b.terminate(Terminator::Return(Some(next)));
    b.switch_to(otherwise);
    b.terminate(Terminator::Return(Some(x)));
    let mut f = b.finish();
    assert_eq!(f.reuse(), 1, "the sum reads 5");
    f.fold();
    let then = f.blocks.iter().nth(1).unwrap().1;
    let Terminator::Return(Some(returned)) = then.term else { panic!("it returns") };
    assert_eq!(f.constant(returned), Some((Scalar::I64, 6)));
    let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
    unit.functions.push(f);
    assert_eq!(unit.verify(&origins), Ok(()));
}
