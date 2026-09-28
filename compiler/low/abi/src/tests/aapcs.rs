//! 32-bit Arm's conventions (AAPCS, section 6.5): floats backfilling VFP registers, soft floats
//! in general registers, records straddling registers and the stack; and wide results, which
//! every convention returns as the record of their halves.

use vun_lir::{AbiType, Ext, Record, Scalar};

use super::{AAPCS64, SYSV, c, int};
use crate::{Carrier, Convention, Loc, Part, Passing, Registers, Rest, Ret, Rules, Unpassable};

/// `r0` to `r3`; `s0` to `s15`, a double named by its first single (`d1` is `s2`).
const CORE: &[u8] = &[0, 1, 2, 3];
const SINGLES: &[u8] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
const REGISTERS: Registers<u8, u8> = Registers {
    int_args: CORE,
    float_args: SINGLES,
    int_results: CORE,
    float_results: SINGLES,
    result_address: None,
    preserved: &[],
    reserved: &[],
};
pub(super) const VFP: Convention<u8, u8> = Convention { rules: &Rules::AAPCS_VFP, registers: REGISTERS };
pub(super) const SOFT: Convention<u8, u8> = Convention { rules: &Rules::AAPCS, registers: REGISTERS };

const I32: AbiType = AbiType::Int(Scalar::I32, Ext::Sign);
const F32: AbiType = AbiType::Scalar(Scalar::F32);
const F64: AbiType = AbiType::Scalar(Scalar::F64);
/// A 64-bit integer, or a soft `f64`, as legalization passes it on a 32-bit machine.
const PAIR: AbiType = AbiType::Wide { half: Scalar::I32, ext: Ext::Zero };

fn float(k: u8) -> Loc<Carrier<u8, u8>> {
    Loc::Reg(Carrier::Float(k))
}

fn stack(offset: u64, bytes: u64) -> Loc<Carrier<u8, u8>> {
    Loc::Stack { offset, bytes }
}

fn params(convention: &Convention<u8, u8>, params: Vec<AbiType>) -> Vec<Loc<Carrier<u8, u8>>> {
    Passing::of(&c(params, None), convention).unwrap().params
}

/// A record of `count` fields of `scalar`, one after another.
fn fields(scalar: Scalar, count: u64) -> Record {
    let bytes = scalar.bytes(32);
    Record { size: count * bytes, align: bytes, fields: (0..count).map(|k| (k * bytes, scalar)).collect() }
}

fn parts(regs: impl IntoIterator<Item = Carrier<u8, u8>>, bytes: u64) -> Vec<Part<Carrier<u8, u8>>> {
    let regs = regs.into_iter().enumerate();
    regs.map(|(k, reg)| Part { reg, offset: k as u64 * bytes, bytes }).collect()
}

#[test]
fn a_float_fills_back_the_single_a_double_skipped() {
    // `s0`, `d1`, then `s1`, which aligning the double left free.
    assert_eq!(params(&VFP, vec![F32, F64, F32]), [float(0), float(2), float(1)]);
}

#[test]
fn a_float_on_the_stack_closes_the_vfp_registers() {
    // Seven doubles and a float leave `s15` alone: the double after goes on the stack, and so
    // does the float after it, `s15` no longer taken (rule C.2).
    let mut given = vec![F64; 7];
    given.extend([F32, F64, F32]);
    let placed = params(&VFP, given);
    assert_eq!(placed[6..], [float(12), float(14), stack(0, 8), stack(8, 4)]);
}

#[test]
fn soft_floats_travel_in_core_registers_and_pairs_start_even() {
    assert_eq!(params(&SOFT, vec![F32, PAIR, I32]), [int(0), int(2), int(3), stack(0, 4)]);
    // Three words taken: the pair takes 8 aligned bytes of the stack and closes `r3` (C.3, C.6).
    assert_eq!(
        params(&SOFT, vec![I32, I32, I32, PAIR, I32]),
        [int(0), int(1), int(2), stack(0, 4), stack(4, 4), stack(8, 4)]
    );
}

#[test]
fn a_scalar_wider_than_a_register_is_legalizations_to_split() {
    assert_eq!(Passing::of(&c(vec![F64], None), &SOFT), Err(Unpassable::Wider(8)));
    assert!(!Rules::AAPCS.fits(Scalar::F64) && !Rules::AAPCS.fits(Scalar::I64));
    assert!(Rules::AAPCS_VFP.fits(Scalar::F64) && Rules::AAPCS.fits(Scalar::F32));
    assert!(!Rules::SYSV.fits(Scalar::I128) && Rules::SYSV.fits(Scalar::I64));
}

#[test]
fn a_record_straddles_the_last_registers_and_the_stack_while_it_is_empty() {
    let words = fields(Scalar::I32, 5);
    let placed = params(&SOFT, vec![I32, AbiType::Record(words.clone()), I32]);
    let head = parts((1..4).map(Carrier::Int), 4);
    let rest = Rest { from: 12, offset: 0, bytes: 8 };
    assert_eq!(placed, [int(0), Loc::Split { parts: head, rest: Some(rest) }, stack(8, 4)]);
    // A double on the stack first: the record is copied there whole, and closes `r1` to `r3`.
    let mut given = vec![F64; 8];
    given.extend([F64, I32, AbiType::Record(words), I32]);
    let placed = params(&VFP, given);
    assert_eq!(placed[8..], [stack(0, 8), int(0), Loc::Copied { offset: 8, size: 20 }, stack(28, 4)]);
}

#[test]
fn an_aligned_record_starts_at_an_even_register_before_straddling() {
    let aligned = Record { size: 16, align: 8, fields: vec![(0, Scalar::I64), (8, Scalar::I64)] };
    let placed = params(&SOFT, vec![I32, AbiType::Record(aligned)]);
    let rest = Rest { from: 8, offset: 0, bytes: 8 };
    let split = Loc::Split { parts: parts((2..4).map(Carrier::Int), 4), rest: Some(rest) };
    assert_eq!(placed, [int(0), split]);
}

#[test]
fn a_homogeneous_aggregate_takes_its_registers_whatever_its_size() {
    let doubles = AbiType::Record(fields(Scalar::F64, 3));
    let split =
        |step: u8| Loc::Split { parts: parts((0..3).map(|k| Carrier::Float(k * step)), 8), rest: None };
    assert_eq!(params(&VFP, vec![doubles.clone()]), [split(2)]);
    // Under AAPCS64 too, past the 16 bytes a record of general registers holds.
    assert_eq!(params(&AAPCS64, vec![doubles.clone()]), [split(1)]);
    // Soft, it is five words and one on the stack.
    let head = parts((0..4).map(Carrier::Int), 4);
    let rest = Rest { from: 16, offset: 0, bytes: 8 };
    assert_eq!(params(&SOFT, vec![doubles]), [Loc::Split { parts: head, rest: Some(rest) }]);
}

#[test]
fn only_a_record_of_a_word_or_of_vfp_registers_comes_back_in_registers() {
    let result =
        |convention, record| Passing::of(&c(Vec::new(), Some(AbiType::Record(record))), convention).unwrap();
    let word = result(&SOFT, fields(Scalar::I16, 2));
    assert_eq!(word.ret, Ret::Split(parts([Carrier::Int(0)], 4)));
    let two = result(&SOFT, fields(Scalar::I32, 2));
    assert_eq!((two.ret, two.params), (Ret::Memory, vec![int(0)]));
    let floats = result(&VFP, fields(Scalar::F32, 2));
    assert_eq!(floats.ret, Ret::Split(parts([Carrier::Float(0), Carrier::Float(1)], 4)));
}

#[test]
fn a_wide_result_comes_back_as_its_halves_written_to_the_callers_place() {
    let wide = |half| Some(AbiType::Wide { half, ext: Ext::Sign });
    let sysv = Passing::of(&c(Vec::new(), wide(Scalar::I64)), &SYSV).unwrap();
    assert_eq!(
        (sysv.ret, sysv.params),
        (Ret::Split(parts([0, 1].map(Carrier::Int), 8)), vec![Loc::Returned])
    );
    let soft = Passing::of(&c(Vec::new(), wide(Scalar::I32)), &SOFT).unwrap();
    assert_eq!(soft.ret, Ret::Split(parts([0, 1].map(Carrier::Int), 4)));
}
