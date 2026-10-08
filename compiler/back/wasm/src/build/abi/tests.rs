use vun_lir::{AbiType, Conv, Ext, Record, Scalar, Signature};

use super::Passing;

fn record(size: u64, align: u64) -> AbiType {
    let fields = (0..size / 8).map(|k| (k * 8, Scalar::I64)).collect();
    AbiType::Record(Record { size, align, fields })
}

fn passing(params: Vec<AbiType>, result: Option<AbiType>) -> Passing {
    let signature = Signature { conv: Conv::C, params, result, variadic: None, convention: None };
    Passing::of(&signature).expect("passable")
}

#[test]
fn a_tail_call_passes_its_copies_over_the_ones_its_caller_received_in_order() {
    let int = AbiType::Int(Scalar::I32, Ext::Sign);
    let caller = passing(vec![int.clone(), record(32, 8), record(16, 8)], None);
    let sized = passing(vec![record(24, 8), record(16, 8)], None);
    assert_eq!(sized.reuse(&caller), Some(vec![1, 2]));
    let through = passing(vec![record(24, 8), int], Some(record(32, 8)));
    assert_eq!(through.reuse(&caller), Some(vec![1]));
    // A caller whose result is written through an address has it as its first parameter.
    let giving = passing(vec![record(32, 8)], Some(record(32, 8)));
    assert_eq!(passing(vec![record(16, 8)], None).reuse(&giving), Some(vec![1]));
}

#[test]
fn a_tail_call_whose_copy_fits_none_of_the_callers_is_refused() {
    let caller = passing(vec![record(16, 8)], None);
    assert_eq!(passing(vec![record(32, 8)], None).reuse(&caller), None);
    assert_eq!(passing(vec![record(16, 16)], None).reuse(&caller), None);
    assert_eq!(passing(vec![record(16, 8), record(16, 8)], None).reuse(&caller), None);
}

#[test]
fn a_tail_call_takes_the_first_slot_not_taken_that_holds_a_copy_not_the_first_after_the_last() {
    let caller = passing(vec![record(16, 8), record(32, 8)], None);
    let swapped = passing(vec![record(32, 8), record(16, 8)], None);
    assert_eq!(swapped.reuse(&caller), Some(vec![1, 0]));
}
