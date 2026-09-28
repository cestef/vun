use vun_intern::Symbol;
use vun_origin::OriginTable;
use vun_target::Endian;

use super::{Half, Legalize, Unlegalized};
use vun_lir::func::{Conv, Linkage};
use vun_lir::testing::origin;
use vun_lir::{AbiType, BinOp, Builder, Cond, Ext, Scalar, Signature, Terminator, Unit};

/// A function of two `i128`s under `conv`: `((x + y) * x << y) / y`.
fn arithmetic(conv: Conv, origins: &OriginTable) -> Unit {
    let at = origin(origins);
    let wide = AbiType::Int(Scalar::I128, Ext::Sign);
    let signature =
        Signature { conv, params: vec![wide.clone(), wide.clone()], result: Some(wide), variadic: None };
    let mut b = Builder::new(Linkage::Export(Symbol::intern("f")), signature, at);
    let [x, y] = [b.params(b.current())[0], b.params(b.current())[1]];
    let sum = b.binary(BinOp::Add, x, y, at);
    let product = b.binary(BinOp::Mul, sum, x, at);
    let shifted = b.binary(BinOp::Shl, product, y, at);
    let quotient = b.binary(BinOp::SDiv, shifted, y, at);
    b.terminate(Terminator::Return(Some(quotient)));
    let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
    unit.functions.push(b.finish());
    unit
}

/// Whether a value of `unit` is wider than `widest`.
fn wider(unit: &Unit, widest: Scalar) -> bool {
    unit.functions
        .iter()
        .any(|(_, f)| f.values.iter().any(|(_, v)| v.ty.is_int() && v.ty.bits(0) > widest.bits(0)))
}

#[test]
fn wide_integers_become_halves_and_division_a_helper() {
    let origins = OriginTable::new();
    let mut unit = arithmetic(Conv::C, &origins);
    Legalize { widest: Scalar::I64, endian: Endian::Little, half: Half::Computed }.apply(&mut unit).unwrap();
    assert_eq!(unit.verify(&origins), Ok(()));
    assert!(!wider(&unit, Scalar::I64), "{unit}");
    // The division helper was added, each parameter became a pair of halves passed together
    // and the result written through an address as its halves.
    assert_eq!(unit.functions.len(), 2);
    let f = &unit.functions.iter().next().unwrap().1;
    assert!(
        f.signature.params.iter().all(|p| matches!(p, AbiType::Wide { limb: Scalar::I64, limbs: 2, .. }))
    );
    assert_eq!(f.signature.values().len(), 5);
    assert!(f.signature.through());
}

#[test]
fn integers_four_times_the_widest_become_four_limbs_under_vun_s_convention_alone() {
    let origins = OriginTable::new();
    let thumb = Legalize { widest: Scalar::I32, endian: Endian::Little, half: Half::Computed };
    let mut unit = Unit { pointer_bits: 32, ..arithmetic(Conv::Vun, &origins) };
    thumb.apply(&mut unit).unwrap();
    assert_eq!(unit.verify(&origins), Ok(()));
    assert!(!wider(&unit, Scalar::I32), "{unit}");
    let f = &unit.functions.iter().next().unwrap().1;
    assert!(
        f.signature.params.iter().all(|p| matches!(p, AbiType::Wide { limb: Scalar::I32, limbs: 4, .. }))
    );
    assert_eq!(f.signature.values().len(), 9);
}

/// A function of two `f16`s under `conv`: their sum where the first is less, else the second.
fn half(conv: Conv, origins: &OriginTable) -> Unit {
    let at = origin(origins);
    let half = AbiType::Scalar(Scalar::F16);
    let signature =
        Signature { conv, params: vec![half.clone(), half.clone()], result: Some(half), variadic: None };
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
    PROMOTED.apply(&mut unit).unwrap();
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
    assert_eq!(PROMOTED.apply(&mut unit).map_err(|e| e.why), Err(Unlegalized::HalfAbi));
}
