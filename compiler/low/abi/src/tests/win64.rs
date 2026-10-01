//! Microsoft x64: the position of a value decides its register in either class, and the records
//! and variadic floats it passes the way Microsoft documents. The registers are numbered as the
//! oracle reads them: `rcx`, `rdx`, `r8` and `r9` are 3, 2, 4 and 5, and `xmm0` to `xmm3` are 0
//! to 3.

use vun_lir::{AbiType, Alias, Conv, Ext, Record, Scalar, Signature};

use super::{WIN64, c, int};
use crate::{Carrier, Loc, Part, Passing, Ret, Unpassable};

const I64: AbiType = AbiType::Int(Scalar::I64, Ext::Zero);
const I32: AbiType = AbiType::Int(Scalar::I32, Ext::Sign);
const F64: AbiType = AbiType::Scalar(Scalar::F64);
const PTR: AbiType = AbiType::Address(Alias::Any);
const RCX: u8 = 3;
const RDX: u8 = 2;
const R8: u8 = 4;
const R9: u8 = 5;

fn float(k: u8) -> Loc<Carrier<u8, u8>> {
    Loc::Reg(Carrier::Float(k))
}

fn stack(offset: u64) -> Loc<Carrier<u8, u8>> {
    Loc::Stack { offset, bytes: 8 }
}

fn record(size: u64, fields: Vec<(u64, Scalar)>) -> AbiType {
    AbiType::Record(Record { size, align: size.min(8), fields })
}

#[test]
fn a_values_position_decides_its_register_in_either_class() {
    let passing = Passing::of(&c(vec![I64, F64, I64, F64, I64, F64], None), &WIN64).unwrap();
    // The fifth value is past the four registers: on the stack, above the 32 bytes of shadow space.
    assert_eq!(passing.params, [int(RCX), float(1), int(R8), float(3), stack(32), stack(40)]);
    assert_eq!(passing.stack, 48);
}

#[test]
fn a_call_leaves_the_shadow_space_whatever_it_passes() {
    assert_eq!(Passing::of(&c(Vec::new(), None), &WIN64).unwrap().stack, 32);
    assert_eq!(Passing::of(&c(vec![I32], None), &WIN64).unwrap().stack, 32);
}

#[test]
fn a_record_of_a_power_of_two_up_to_a_word_travels_whole_in_a_general_register() {
    let double = record(8, vec![(0, Scalar::F64)]);
    let pair = record(4, vec![(0, Scalar::I16), (2, Scalar::I16)]);
    let passing = Passing::of(&c(vec![double, pair, I32], None), &WIN64).unwrap();
    let whole =
        |k, bytes| Loc::Split { parts: vec![Part { reg: Carrier::Int(k), offset: 0, bytes }], rest: None };
    // The class of its fields does not matter, and each takes the register of its position.
    assert_eq!(passing.params, [whole(RCX, 8), whole(RDX, 4), int(R8)]);
}

#[test]
fn any_other_record_is_copied_by_the_caller_and_its_address_passed() {
    let three = record(3, vec![(0, Scalar::I8), (1, Scalar::I8), (2, Scalar::I8)]);
    let sixteen = record(16, vec![(0, Scalar::I64), (8, Scalar::I64)]);
    let passing = Passing::of(&c(vec![three, sixteen, I32], None), &WIN64).unwrap();
    let by_address = |size, at| Loc::Indirect { size, at: Box::new(int(at)) };
    assert_eq!(passing.params, [by_address(3, RCX), by_address(16, RDX), int(R8)]);
}

#[test]
fn a_result_in_memory_takes_the_first_register_and_moves_the_arguments_on() {
    let big = Some(record(16, vec![(0, Scalar::I64), (8, Scalar::I64)]));
    let passing = Passing::of(&c(vec![I64, F64], big), &WIN64).unwrap();
    // The address is the first value, so the arguments are the second and third.
    assert_eq!(passing.ret, Ret::Memory);
    assert_eq!(passing.params, [int(RDX), float(2), int(RCX)]);
}

#[test]
fn a_small_record_result_comes_back_in_rax_even_of_floats() {
    let double = Some(record(8, vec![(0, Scalar::F64)]));
    let passing = Passing::of(&c(Vec::new(), double), &WIN64).unwrap();
    assert_eq!(passing.ret, Ret::Split(vec![Part { reg: Carrier::Int(0), offset: 0, bytes: 8 }]));
}

#[test]
fn a_float_among_the_variadic_arguments_travels_as_an_integer_in_its_position() {
    // `printf(fmt, 7, 3.5, 2.5)`: the float after the fixed argument is the integer of its bits.
    let printf =
        Signature { conv: Conv::C, params: vec![PTR, I32, F64, F64], result: Some(I32), variadic: Some(1) };
    let passing = Passing::of(&printf, &WIN64).unwrap();
    assert_eq!(passing.params, [int(RCX), int(RDX), int(R8), int(R9)]);
    // Fixed, the same float is in a floating-point register of its position.
    let fixed = Signature { variadic: None, ..printf };
    assert_eq!(Passing::of(&fixed, &WIN64).unwrap().params, [int(RCX), int(RDX), float(2), float(3)]);
}

#[test]
fn an_integer_wider_than_a_register_is_no_c_integer() {
    let wide = AbiType::Wide { limb: Scalar::I64, limbs: 2, ext: Ext::Sign };
    assert_eq!(Passing::of(&c(vec![wide], None), &WIN64), Err(Unpassable::Integer(16)));
}
