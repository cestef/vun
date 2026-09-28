//! arm64: fusion, callee-saved registers for loops, results in dying operands' registers, each
//! instruction decoded by an independent decoder to the text the listing shows.

use vun_lir::{BinOp, Cond, Scalar, Terminator};
use yaxpeax_arch::{Decoder, U8Reader};
use yaxpeax_arm::armv8::a64::InstDecoder;

use crate::Arm;
use vun_native::Code;
use vun_native::testing::{compiled, sum, text, to};

/// The listing of `code`, each instruction checked against the decoder's reading of its word.
fn checked(code: &Code<Arm>) -> Vec<String> {
    let listed = text(code);
    for (k, ours) in listed.iter().enumerate() {
        let word = &code.code.bytes[k * 4..k * 4 + 4];
        let theirs = InstDecoder::default().decode(&mut U8Reader::new(word)).unwrap().to_string();
        let linked = code.code.relocs.iter().any(|r| r.offset == (k * 4) as u64);
        let mnemonic = |t: &str| t.split(' ').next().map(str::to_owned);
        assert!(
            theirs == *ours || (linked || ours.starts_with('b')) && mnemonic(&theirs) == mnemonic(ours),
            "{ours} / {theirs}"
        );
    }
    listed
}

#[test]
fn a_branch_on_a_comparison_jumps_on_its_flags() {
    // `if n < 10 { 1 } else { 2 }`.
    let code = compiled::<Arm>(|b, n, origin| {
        let ten = b.constant(Scalar::I64, 10, origin);
        let less = b.compare(Cond::SLt, n, ten, origin);
        let (small, large) = (b.block(&[], origin), b.block(&[], origin));
        b.terminate(Terminator::Branch { cond: less, then: to(small), otherwise: to(large) });
        for (block, value) in [(small, 1), (large, 2)] {
            b.switch_to(block);
            let v = b.constant(Scalar::I64, value, origin);
            b.terminate(Terminator::Return(Some(v)));
        }
    });
    let text = checked(&code);
    // The constant is an immediate, the condition never materialized.
    assert!(text.iter().any(|i| i == "cmp x0, #0xa"), "{text:#?}");
    // The block taken where it holds is emitted next: the branch jumps where it does not.
    assert!(text.iter().any(|i| i.starts_with("b.ge ")), "{text:#?}");
    assert!(!text.iter().any(|i| i.starts_with("b ")), "{text:#?}");
    assert!(!text.iter().any(|i| i.starts_with("cset")), "{text:#?}");
}

#[test]
fn a_loop_keeps_its_values_in_registers() {
    let text = checked(&compiled::<Arm>(|b, n, origin| sum(b, n, origin, false)));
    // Nothing is stored in the frame or loaded from it.
    assert!(!text.iter().any(|i| i.contains("[x29")), "{text:#?}");
}

#[test]
fn values_live_across_a_call_take_callee_saved_registers() {
    let text = checked(&compiled::<Arm>(|b, n, origin| sum(b, n, origin, true)));
    // A register the call preserves saved below the frame pointer and restored.
    assert!(text.iter().any(|i| i.starts_with("stur x19, [x29, #-")), "{text:#?}");
    assert!(text.iter().any(|i| i.starts_with("ldur x19, [x29, #-")), "{text:#?}");
}

#[test]
fn a_dying_operand_s_register_holds_the_result() {
    // `(n + 1) * 3`: each result overwrites the operand it no longer needs, and constants are
    // immediates where a form takes them.
    let code = compiled::<Arm>(|b, n, origin| {
        let one = b.constant(Scalar::I64, 1, origin);
        let three = b.constant(Scalar::I64, 3, origin);
        let sum = b.binary(BinOp::Add, n, one, origin);
        let product = b.binary(BinOp::Mul, sum, three, origin);
        b.terminate(Terminator::Return(Some(product)));
    });
    let text = checked(&code);
    let body: Vec<&str> = text.iter().skip(5).map(String::as_str).collect();
    assert_eq!(
        body,
        [
            "add x0, x0, #0x1",
            "mov x1, #0x3",
            "mul x0, x0, x1",
            "mov sp, x29",
            "ldp x29, x30, [sp], #0x10",
            "ret"
        ]
    );
}
