mod clang;

use vun_lir::{AbiType, Conv, Ext, Record, Scalar, Signature};

use crate::{Carrier, Convention, Loc, Part, Passing, Registers, Ret, Rules};

/// Registers named by numbers: System V's six general and eight floating-point argument registers,
/// two result registers of each class.
const SYSV: Convention<u8, u8> = Convention {
    rules: &Rules::SYSV,
    registers: Registers {
        int_args: &[0, 1, 2, 3, 4, 5],
        float_args: &[0, 1, 2, 3, 4, 5, 6, 7],
        int_results: &[0, 1],
        float_results: &[0, 1],
        result_address: None,
        preserved: &[],
        reserved: &[],
    },
};

const EIGHT: &[u8] = &[0, 1, 2, 3, 4, 5, 6, 7];
const AAPCS64: Convention<u8, u8> = Convention {
    rules: &Rules::AAPCS64,
    registers: Registers {
        int_args: EIGHT,
        float_args: EIGHT,
        int_results: EIGHT,
        float_results: EIGHT,
        result_address: Some(8),
        preserved: &[],
        reserved: &[],
    },
};
const DARWIN: Convention<u8, u8> = Convention { rules: &Rules::DARWIN_ARM64, ..AAPCS64 };

const I64: AbiType = AbiType::Int(Scalar::I64, Ext::Zero);
const WIDE: AbiType = AbiType::Wide { half: Scalar::I64, ext: Ext::Sign };

fn c(params: Vec<AbiType>, result: Option<AbiType>) -> Signature {
    Signature { conv: Conv::C, params, result }
}

type Place = Loc<Carrier<u8, u8>>;

fn int(k: u8) -> Place {
    Loc::Reg(Carrier::Int(k))
}

fn words(k: u8) -> Record {
    Record {
        size: u64::from(k) * 8,
        align: 8,
        fields: (0..u64::from(k)).map(|i| (i * 8, Scalar::I64)).collect(),
    }
}

#[test]
fn a_record_past_two_eightbytes_is_copied_whole_and_takes_no_register() {
    let passing = Passing::of(&c(vec![AbiType::Record(words(3)), I64], None), &SYSV).unwrap();
    assert_eq!(passing.params, [Loc::Copied { offset: 0, size: 24 }, int(0)]);
    let passing = Passing::of(&c(vec![AbiType::Record(words(3)), I64], None), &AAPCS64).unwrap();
    assert_eq!(passing.params, [Loc::Indirect { size: 24, at: Box::new(int(0)) }, int(1)]);
}

#[test]
fn eightbytes_travel_by_class() {
    let mixed =
        Record { size: 16, align: 8, fields: vec![(0, Scalar::F64), (8, Scalar::I32), (12, Scalar::F32)] };
    let passing = Passing::of(&c(vec![AbiType::Record(mixed)], None), &SYSV).unwrap();
    let parts = vec![
        Part { reg: Carrier::Float(0), offset: 0, bytes: 8 },
        Part { reg: Carrier::Int(0), offset: 8, bytes: 8 },
    ];
    assert_eq!(passing.params, [Loc::Split(parts)]);
    assert_eq!(passing.floats, 1);
}

#[test]
fn a_record_of_floats_of_one_type_takes_a_register_a_field_under_aapcs64() {
    let floats =
        Record { size: 12, align: 4, fields: vec![(0, Scalar::F32), (4, Scalar::F32), (8, Scalar::F32)] };
    let passing = Passing::of(&c(vec![AbiType::Record(floats)], None), &AAPCS64).unwrap();
    let parts = (0..3).map(|k| Part { reg: Carrier::Float(k), offset: u64::from(k) * 4, bytes: 4 }).collect();
    assert_eq!(passing.params, [Loc::Split(parts)]);
}

#[test]
fn a_record_finding_too_few_registers_closes_its_class_only_under_aapcs64() {
    let mut params = vec![I64; 7];
    params.extend([AbiType::Record(words(2)), I64]);
    let sysv = Passing::of(&c(params.clone(), None), &SYSV).unwrap();
    // Six registers: the seventh integer is on the stack, the record after it too, whole.
    assert_eq!(
        sysv.params[6..],
        [
            Loc::Stack { offset: 0, bytes: 8 },
            Loc::Copied { offset: 8, size: 16 },
            Loc::Stack { offset: 24, bytes: 8 }
        ]
    );
    let aapcs = Passing::of(&c(params, None), &AAPCS64).unwrap();
    assert_eq!(aapcs.params[7..], [Loc::Copied { offset: 0, size: 16 }, Loc::Stack { offset: 16, bytes: 8 }]);
}

#[test]
fn a_wide_integer_takes_two_registers_or_16_aligned_stack() {
    let mut params = vec![I64; 3];
    params.extend([WIDE, I64, WIDE, I64]);
    let passing = Passing::of(&c(params, None), &SYSV).unwrap();
    assert_eq!(passing.params[3..6], [int(3), int(4), int(5)]);
    // One register left: the second goes on the stack, and the register stays free.
    let stack = |offset| Loc::Stack { offset, bytes: 8 };
    assert_eq!(passing.params[6..], [stack(0), stack(8), stack(16)]);
    assert_eq!(passing.stack, 32);
}

#[test]
fn a_wide_integer_takes_an_even_register_pair_under_aapcs64() {
    let passing = Passing::of(&c(vec![I64, WIDE, I64, I64, WIDE, WIDE, I64], None), &AAPCS64).unwrap();
    // `x1` is skipped for the first pair; the third finds no pair left and takes 16 bytes of the
    // stack, after which no general register is used.
    assert_eq!(passing.params[..7], [int(0), int(2), int(3), int(4), int(5), int(6), int(7)]);
    let stack = |offset| Loc::Stack { offset, bytes: 8 };
    assert_eq!(passing.params[7..], [stack(0), stack(8), stack(16)]);
}

#[test]
fn apple_packs_a_c_calls_stack_arguments_by_their_own_size() {
    let narrow = |s| AbiType::Int(s, Ext::Zero);
    let mut params = vec![narrow(Scalar::I32); 8];
    params.extend([narrow(Scalar::I8), narrow(Scalar::I8), narrow(Scalar::I16), narrow(Scalar::I32)]);
    let stacked = |convention| {
        let passing = Passing::of(&c(params.clone(), None), convention).unwrap();
        let at = passing.params.iter().filter_map(|loc| match *loc {
            Loc::Stack { offset, bytes } => Some((offset, bytes)),
            _ => None,
        });
        (at.collect::<Vec<_>>(), passing.stack)
    };
    assert_eq!(stacked(&DARWIN), (vec![(0, 1), (1, 1), (2, 2), (4, 4)], 16));
    assert_eq!(stacked(&AAPCS64), (vec![(0, 8), (8, 8), (16, 8), (24, 8)], 32));
}

#[test]
fn a_result_in_memory_takes_the_first_register_or_one_of_its_own() {
    let big = Some(AbiType::Record(words(3)));
    let sysv = Passing::of(&c(vec![I64], big.clone()), &SYSV).unwrap();
    assert_eq!((sysv.ret, sysv.params), (Ret::Memory, vec![int(1), int(0)]));
    let aapcs = Passing::of(&c(vec![I64], big), &AAPCS64).unwrap();
    assert_eq!((aapcs.ret, aapcs.params), (Ret::Memory, vec![int(0), Loc::Reg(Carrier::Int(8))]));
}

#[test]
fn a_small_record_result_comes_back_in_registers_and_its_place_is_the_callers() {
    let passing = Passing::of(&c(Vec::new(), Some(AbiType::Record(words(2)))), &SYSV).unwrap();
    let parts = (0..2).map(|k| Part { reg: Carrier::Int(k), offset: u64::from(k) * 8, bytes: 8 }).collect();
    assert_eq!((passing.ret, passing.params), (Ret::Split(parts), vec![Loc::Returned]));
}

#[test]
fn a_record_after_packed_scalars_is_word_aligned_on_the_stack() {
    let byte = AbiType::Int(Scalar::I8, Ext::Zero);
    let pair = Record { size: 16, align: 8, fields: vec![(0, Scalar::I64), (8, Scalar::I64)] };
    let mut params = vec![byte; 9];
    params.push(AbiType::Record(pair));
    let passing = Passing::of(&c(params, None), &DARWIN).unwrap();
    assert_eq!(passing.params[8], Loc::Stack { offset: 0, bytes: 1 });
    assert_eq!(passing.params[9], Loc::Copied { offset: 8, size: 16 });
}
