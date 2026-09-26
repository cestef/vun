use vun_intern::Symbol;
use vun_origin::OriginTable;
use vun_target::Endian;

use super::{Half, Legalize, Unlegalized};
use crate::func::{Conv, Linkage};
use crate::tests::origin;
use crate::{AbiType, BinOp, Builder, Cond, Ext, Scalar, Signature, Terminator, Unit};

#[test]
fn wide_integers_become_halves_and_division_a_helper() {
    let origins = OriginTable::new();
    let at = origin(&origins);
    let wide = AbiType::Int(Scalar::I128, Ext::Sign);
    let signature = Signature { conv: Conv::C, params: vec![wide.clone(), wide.clone()], result: Some(wide) };
    let mut b = Builder::new(Linkage::Export(Symbol::intern("f")), signature, at);
    let [x, y] = [b.params(b.current())[0], b.params(b.current())[1]];
    let sum = b.binary(BinOp::Add, x, y, at);
    let product = b.binary(BinOp::Mul, sum, x, at);
    let shifted = b.binary(BinOp::Shl, product, y, at);
    let quotient = b.binary(BinOp::SDiv, shifted, y, at);
    b.terminate(Terminator::Return(Some(quotient)));
    let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
    unit.functions.push(b.finish());
    unit.legalize(Legalize { widest: Scalar::I64, endian: Endian::Little, half: Half::Computed }).unwrap();
    assert_eq!(unit.verify(&origins), Ok(()));
    let wide_left = unit.functions.iter().any(|(_, f)| f.values.iter().any(|(_, v)| v.ty == Scalar::I128));
    assert!(!wide_left, "{unit}");
    // The division helper was added, and the result became a record of halves.
    assert_eq!(unit.functions.len(), 2);
    let f = &unit.functions.iter().next().unwrap().1;
    assert_eq!(f.signature.params.len(), 4);
    assert!(f.signature.through());
}

/// A function of two `f16`s under `conv`: their sum where the first is less, else the second.
fn half(conv: Conv, origins: &OriginTable) -> Unit {
    let at = origin(origins);
    let half = AbiType::Scalar(Scalar::F16);
    let signature = Signature { conv, params: vec![half.clone(), half.clone()], result: Some(half) };
    let mut b = Builder::new(Linkage::Export(Symbol::intern("f")), signature, at);
    let [x, y] = [b.params(b.current())[0], b.params(b.current())[1]];
    let sum = b.binary(BinOp::FAdd, x, y, at);
    let less = b.compare(Cond::FLt, x, y, at);
    let chosen = b.select(less, sum, y, at);
    b.terminate(Terminator::Return(Some(chosen)));
    let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
    unit.functions.push(b.finish());
    unit
}

const PROMOTED: Legalize = Legalize { widest: Scalar::I64, endian: Endian::Little, half: Half::Promoted };

#[test]
fn a_promoted_f16_becomes_its_bits_computed_in_f32() {
    let origins = OriginTable::new();
    let mut unit = half(Conv::Vun, &origins);
    unit.legalize(PROMOTED).unwrap();
    assert_eq!(unit.verify(&origins), Ok(()));
    let f = &unit.functions.iter().next().unwrap().1;
    assert!(!f.values.iter().any(|(_, v)| v.ty == Scalar::F16), "{unit}");
    assert!(f.values.iter().any(|(_, v)| v.ty == Scalar::F32), "{unit}");
    assert_eq!(f.signature.params[0], AbiType::Int(Scalar::I16, Ext::Zero));
}

#[test]
fn a_promoted_f16_is_not_passed_under_the_c_convention() {
    let origins = OriginTable::new();
    let mut unit = half(Conv::C, &origins);
    assert_eq!(unit.legalize(PROMOTED).map_err(|e| e.why), Err(Unlegalized::HalfAbi));
}
