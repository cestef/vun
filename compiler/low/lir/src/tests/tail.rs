//! Tail calls: where they may stand, and how a block is told to end in one.

use vun_index::Idx;
use vun_origin::OriginTable;

use crate::func::{Conv, Linkage, Signature};
use crate::testing::origin;
use crate::verify::Reason;
use crate::{Builder, FuncId, InstKind, Scalar, Terminator, Unit};

#[test]
fn the_verifier_accepts_a_tail_call_only_as_the_last_instruction_returning_its_value() {
    use crate::{CallKind, Callee};
    let origins = OriginTable::new();
    let o = origin(&origins);
    let verify = |kind: CallKind, term: fn(Option<crate::Value>) -> Terminator| {
        let int = crate::AbiType::Int(Scalar::I64, crate::Ext::Zero);
        let signature = Signature {
            conv: Conv::Vun,
            params: vec![int.clone()],
            result: Some(int),
            variadic: None,
            convention: None,
        };
        let mut b = Builder::new(Linkage::Internal, signature, o);
        let n = b.params(b.current())[0];
        let call = InstKind::Call { callee: Callee::Func(FuncId::new(0)), args: vec![n], kind };
        let result = b.push(call, Some(Scalar::I64), o);
        b.terminate(term(result));
        let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
        unit.functions.push(b.finish());
        unit.verify(&origins).map_err(|m| m.reason)
    };
    assert_eq!(verify(CallKind::Plain, Terminator::Return), Ok(()));
    assert_eq!(verify(CallKind::Tail, Terminator::Return), Ok(()));
    assert_eq!(verify(CallKind::Tail, |_| Terminator::Unreachable), Err(Reason::Tail));
}

#[test]
fn a_block_ends_in_a_tail_call_only_when_its_last_instruction_is_one() {
    use crate::{CallKind, Callee};
    let origins = OriginTable::new();
    let o = origin(&origins);
    let ends = |kind: CallKind| {
        let int = crate::AbiType::Int(Scalar::I64, crate::Ext::Zero);
        let signature = Signature {
            conv: Conv::Vun,
            params: vec![int.clone()],
            result: Some(int),
            variadic: None,
            convention: None,
        };
        let mut b = Builder::new(Linkage::Internal, signature, o);
        let n = b.params(b.current())[0];
        let call = InstKind::Call { callee: Callee::Func(FuncId::new(0)), args: vec![n], kind };
        let result = b.push(call, Some(Scalar::I64), o);
        let entry = b.current();
        b.terminate(Terminator::Return(result));
        b.finish().ends_in_tail_call(entry)
    };
    assert!(ends(CallKind::Tail));
    assert!(!ends(CallKind::Plain));
}

#[test]
fn the_verifier_accepts_a_tail_call_writing_a_result_in_memory_only_through_the_address_received() {
    use crate::{CallKind, Callee};
    let origins = OriginTable::new();
    let o = origin(&origins);
    let verify = |received: bool| {
        let int = crate::AbiType::Int(Scalar::I64, crate::Ext::Zero);
        let signature = Signature {
            conv: Conv::Vun,
            params: vec![int],
            result: Some(crate::AbiType::Memory { size: 16, align: 8 }),
            variadic: None,
            convention: None,
        };
        let mut b = Builder::new(Linkage::Internal, signature, o);
        let [n, out] = b.params(b.current())[..] else {
            unreachable!("a parameter and the result's address")
        };
        let slot = b.slot(16, 8, o);
        let local = b.slot_addr(slot, o);
        let call = InstKind::Call {
            callee: Callee::Func(FuncId::new(0)),
            args: vec![n, if received { out } else { local }],
            kind: CallKind::Tail,
        };
        b.push(call, None, o);
        b.terminate(Terminator::Return(None));
        let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
        unit.functions.push(b.finish());
        unit.verify(&origins).map_err(|m| m.reason)
    };
    assert_eq!(verify(true), Ok(()));
    assert_eq!(verify(false), Err(Reason::Tail));
}
