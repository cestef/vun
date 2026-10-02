//! Variadic C calls: where each convention passes the arguments of `...`, as clang does.

use vun_lir::{AbiType, Conv, Ext, Record, Scalar, Signature};

use super::aapcs::VFP;
use super::{AAPCS64, DARWIN, int};
use crate::{Carrier, Loc, Passing, Ret};

const I32: AbiType = AbiType::Int(Scalar::I32, Ext::Sign);
const F32: AbiType = AbiType::Scalar(Scalar::F32);
const F64: AbiType = AbiType::Scalar(Scalar::F64);
const PTR: AbiType = AbiType::Address(vun_lir::Alias::Any);

/// A C function taking `params`, those from `fixed` on variadic.
fn variadic(params: Vec<AbiType>, fixed: usize, result: Option<AbiType>) -> Signature {
    Signature { conv: Conv::C, params, result, variadic: Some(fixed), convention: None }
}

fn stack(offset: u64, bytes: u64) -> Loc<Carrier<u8, u8>> {
    Loc::Stack { offset, bytes }
}

/// A record of two fields of `scalar`.
fn two(scalar: Scalar) -> AbiType {
    AbiType::Record(Record { size: 16, align: 8, fields: vec![(0, scalar), (8, scalar)] })
}

#[test]
fn opens_permissions_travel_on_the_stack_only_on_apple_arm64() {
    let open = variadic(vec![PTR, I32, I32], 2, Some(I32));
    let darwin = Passing::of(&open, &DARWIN).unwrap();
    assert_eq!(darwin.params, [int(0), int(1), stack(0, 8)]);
    assert_eq!(darwin.stack, 16);
    assert_eq!(Passing::of(&open, &AAPCS64).unwrap().params, [int(0), int(1), int(2)]);
}

#[test]
fn apple_arm64_passes_each_variadic_argument_in_the_words_it_takes() {
    // `printf(p, 7, 3.5, (struct { long a, b; }){..}, (struct { double a, b; }){..}, (__int128)9)`.
    let wide = AbiType::Wide { limb: Scalar::I64, limbs: 2, ext: Ext::Sign };
    let printf = variadic(vec![PTR, I32, F64, two(Scalar::I64), two(Scalar::F64), wide], 1, Some(I32));
    let passing = Passing::of(&printf, &DARWIN).unwrap();
    let copied = |offset| Loc::Copied { offset, size: 16 };
    let expected = [int(0), stack(0, 8), stack(8, 8), copied(16), copied(32), stack(48, 8), stack(56, 8)];
    assert_eq!(passing.params, expected);
    assert_eq!(passing.stack, 64);
}

#[test]
fn a_variadic_call_under_aapcs_vfp_passes_everything_as_the_base_standard() {
    // Fixed floats and the result too: `float h(float, ...)` called with an `int`.
    let h = variadic(vec![F32, I32], 1, Some(F32));
    let passing = Passing::of(&h, &VFP).unwrap();
    assert_eq!(passing.params, [int(0), int(1)]);
    assert_eq!(passing.ret, Ret::Reg(Carrier::Int(0)));
    let fixed = Signature { variadic: None, ..h };
    assert_eq!(Passing::of(&fixed, &VFP).unwrap().ret, Ret::Reg(Carrier::Float(0)));
}

#[test]
fn a_call_passing_no_variadic_argument_passes_as_a_fixed_one() {
    let bare = variadic(vec![PTR, I32], 2, Some(I32));
    assert_eq!(Passing::of(&bare, &DARWIN).unwrap().params, [int(0), int(1)]);
}
