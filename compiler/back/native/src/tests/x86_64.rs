//! x86-64: fusion, callee-saved registers for loops, results in dying operands' registers.

use vun_lir::{BinOp, Cond, Scalar, Target, Terminator};

use super::{compiled, text, to};
use crate::x86_64::X86;

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
fn values_live_across_a_loop_take_callee_saved_registers() {
    // The sum of `0..n`, in a loop whose counter and total are block parameters.
    let code = compiled::<X86>(|b, n, origin| {
        let zero = b.constant(Scalar::I64, 0, origin);
        let head = b.block(&[Scalar::I64, Scalar::I64], origin);
        let (body, exit) = (b.block(&[], origin), b.block(&[], origin));
        b.terminate(Terminator::Jump(Target { block: head, args: vec![zero, zero] }));
        b.switch_to(head);
        let [i, total] = [b.params(head)[0], b.params(head)[1]];
        let more = b.compare(Cond::ULt, i, n, origin);
        b.terminate(Terminator::Branch { cond: more, then: to(body), otherwise: to(exit) });
        b.switch_to(body);
        let one = b.constant(Scalar::I64, 1, origin);
        let (next, added) = (b.binary(BinOp::Add, i, one, origin), b.binary(BinOp::Add, total, i, origin));
        b.terminate(Terminator::Jump(Target { block: head, args: vec![next, added] }));
        b.switch_to(exit);
        b.terminate(Terminator::Return(Some(total)));
    });
    let text = text(&code);
    // `rbx` saved below the frame pointer and restored.
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
