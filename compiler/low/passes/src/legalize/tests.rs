use vun_intern::Symbol;
use vun_origin::OriginTable;
use vun_target::{Convention, Endian, Floats};

use super::{Half, Legalize, Unlegalized};
use vun_lir::func::{Conv, Linkage};
use vun_lir::testing::origin;
use vun_lir::{AbiType, BinOp, Builder, CastOp, Cond, Ext, Scalar, Signature, Terminator, Unit};

/// A 64-bit host's backend computing every scalar but `f16`.
const HOST: Legalize = Legalize {
    widest: Scalar::I64,
    endian: Endian::Little,
    half: Half::Computed,
    floats: Floats::ALL,
    convention: Convention::Sysv,
};

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
    HOST.apply(&mut unit).unwrap();
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
    let thumb = Legalize { widest: Scalar::I32, ..HOST };
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

const PROMOTED: Legalize = Legalize { half: Half::Promoted, ..HOST };

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

/// A 32-bit processor computing `f32` alone, whose C functions follow `convention`.
fn single(convention: Convention) -> Legalize {
    Legalize { widest: Scalar::I32, floats: Floats::default().with(32).unwrap(), convention, ..HOST }
}

/// A function under `conv` choosing between two `f64`s by an `i8`, and reading the bits of one:
/// moves of floats alone, or `x + y` where `add`.
fn choose(conv: Conv, add: bool, origins: &OriginTable) -> Unit {
    let at = origin(origins);
    let double = AbiType::Scalar(Scalar::F64);
    let flag = AbiType::Int(Scalar::I8, Ext::Zero);
    let params = vec![double.clone(), double.clone(), flag];
    let signature = Signature { conv, params, result: Some(double), variadic: None };
    let mut b = Builder::new(Linkage::Export(Symbol::intern("f")), signature, at);
    let [x, y, c] = [0, 1, 2].map(|k| b.params(b.current())[k]);
    let zero = b.constant(Scalar::I8, 0, at);
    let set = b.compare(Cond::Ne, c, zero, at);
    let bits = b.cast(CastOp::Bitcast, x, Scalar::I64, at);
    let back = b.cast(CastOp::Bitcast, bits, Scalar::F64, at);
    let other = if add { b.binary(BinOp::FAdd, x, y, at) } else { y };
    let chosen = b.select(set, back, other, at);
    b.terminate(Terminator::Return(Some(chosen)));
    let mut unit = Unit { pointer_bits: 32, ..Unit::default() };
    unit.functions.push(b.finish());
    unit
}

#[test]
fn a_float_the_processor_lacks_is_its_bits_in_limbs() {
    let origins = OriginTable::new();
    let mut unit = choose(Conv::Vun, false, &origins);
    single(Convention::AapcsVfp).apply(&mut unit).unwrap();
    assert_eq!(unit.verify(&origins), Ok(()));
    let f = &unit.functions.iter().next().unwrap().1;
    assert!(!f.values.iter().any(|(_, v)| v.ty == Scalar::F64 || v.ty == Scalar::I64), "{unit}");
    assert!(matches!(f.signature.params[0], AbiType::Wide { limb: Scalar::I32, limbs: 2, .. }));
}

#[test]
fn a_float_held_as_bits_reaches_c_only_in_general_registers() {
    let origins = OriginTable::new();
    let mut hard = choose(Conv::C, false, &origins);
    let refused = single(Convention::AapcsVfp).apply(&mut hard).map_err(|e| e.why);
    assert_eq!(refused, Err(Unlegalized::FloatAbi));
    let mut soft = choose(Conv::C, false, &origins);
    single(Convention::Aapcs).apply(&mut soft).unwrap();
    assert_eq!(soft.verify(&origins), Ok(()));
}

#[test]
fn arithmetic_on_a_float_held_as_bits_is_refused() {
    let origins = OriginTable::new();
    let mut unit = choose(Conv::Vun, true, &origins);
    let refused = single(Convention::AapcsVfp).apply(&mut unit).map_err(|e| e.why);
    assert_eq!(refused, Err(Unlegalized::HeldFloat));
}
