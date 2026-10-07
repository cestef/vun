//! x86-64: fusion, callee-saved registers for loops, results in dying operands' registers.

use vun_lir::func::Conv;
use vun_lir::{AbiType, BinOp, Cond, Ext, Scalar, Terminator};

use vun_target::Convention;

use crate::X86;
use vun_native::testing::{compile_tail, compile_tail_under, compiled, sum, text, to};

#[test]
fn a_branch_on_a_comparison_jumps_on_its_flags() {
    // `if n < 10 { 1 } else { 2 }`.
    let code = compiled::<X86>(|b, n, origin| {
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
    let text = text(&code);
    // The constant is an immediate, the condition never materialized.
    assert!(text.iter().any(|i| i == "cmp rdi, 0xa"), "{text:#?}");
    // The block taken where it holds is emitted next: the branch jumps where it does not.
    assert!(text.iter().any(|i| i.starts_with("jge ")), "{text:#?}");
    assert!(!text.iter().any(|i| i.starts_with("jmp ")), "{text:#?}");
    assert!(!text.iter().any(|i| i.starts_with("set")), "{text:#?}");
}

#[test]
fn a_loop_keeps_its_values_in_registers() {
    let text = text(&compiled::<X86>(|b, n, origin| sum(b, n, origin, false)));
    // Nothing is stored in the frame or loaded from it.
    assert!(!text.iter().any(|i| i.contains("[rbp")), "{text:#?}");
}

#[test]
fn values_live_across_a_call_take_callee_saved_registers() {
    let text = text(&compiled::<X86>(|b, n, origin| sum(b, n, origin, true)));
    // A register the call preserves saved below the frame pointer and restored.
    assert!(text.iter().any(|i| i.starts_with("mov qword [rbp - ") && i.ends_with(", rbx")), "{text:#?}");
    assert!(text.iter().any(|i| i.starts_with("mov rbx, qword [rbp - ")), "{text:#?}");
}

#[test]
fn a_dying_operand_s_register_holds_the_result() {
    // `(n + 1) * 3`: each result overwrites the operand it no longer needs, and constants are
    // immediates.
    let code = compiled::<X86>(|b, n, origin| {
        let one = b.constant(Scalar::I64, 1, origin);
        let three = b.constant(Scalar::I64, 3, origin);
        let sum = b.binary(BinOp::Add, n, one, origin);
        let product = b.binary(BinOp::Mul, sum, three, origin);
        b.terminate(Terminator::Return(Some(product)));
    });
    let text = text(&code);
    let body: Vec<&str> = text.iter().skip(3).map(String::as_str).collect();
    assert_eq!(body, ["add rdi, 0x1", "mov eax, 0x3", "imul rdi, rax", "mov rax, rdi", "leave", "ret"]);
}

#[test]
fn tail_calls_pass_values_in_memory_where_the_function_received_its_own() {
    vun_native::testing::tails_pass_values_in_memory::<crate::X86>("call");
}

#[test]
fn a_tail_call_passes_a_large_record_only_where_the_convention_copies_it_to_the_stack() {
    let record = vun_native::testing::large_record::<crate::X86>();
    let int = AbiType::Int(Scalar::I64, Ext::Sign);
    let code = compile_tail::<crate::X86>(
        Conv::C,
        std::slice::from_ref(&record),
        std::slice::from_ref(&record),
        &int,
    )
    .expect("a record on the stack goes over the one received");
    assert!(!text(&code).iter().any(|i| i.starts_with("call")));
}

#[test]
fn a_tail_call_passes_a_large_record_by_address_under_win64_only_as_one_received() {
    let record = vun_native::testing::large_record::<crate::X86>();
    let int = AbiType::Int(Scalar::I64, Ext::Sign);
    let own = std::slice::from_ref(&record);
    let code = compile_tail_under::<crate::X86>(Convention::Win64, Conv::C, own, own, &int)
        .expect("the copy the caller made is passed on");
    assert!(!text(&code).iter().any(|i| i.starts_with("call")));
    let lacks =
        compile_tail_under::<crate::X86>(Convention::Win64, Conv::C, std::slice::from_ref(&int), own, &int)
            .err();
    assert_eq!(lacks.map(|lack| lack.missing), Some(vun_backend::Missing::TailCall));
}
