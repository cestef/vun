//! The encoder against the bytes the architecture manual gives.

use vun_intern::Symbol;
use vun_lir::func::{Conv, Linkage, Signature};
use vun_lir::{AbiType, BinOp, Block, Builder, Cond, Ext, Scalar, Target, Terminator, Unit, Value};
use vun_origin::{OriginId, OriginTable};
use vun_span::{SourceFiles, Span};

use crate::x86_64::encode::{Alu, Asm};
use crate::x86_64::{Mem, Reg, Rm, Size};

fn encoded(f: impl FnOnce(&mut Asm)) -> Vec<u8> {
    let mut asm = Asm::default();
    f(&mut asm);
    asm.code
}

#[test]
fn register_operands_take_rex_where_extended_or_bytes() {
    assert_eq!(encoded(|a| a.mov_rr(Reg::Rax, Reg::R9)), [0x4c, 0x89, 0xc8]);
    assert_eq!(encoded(|a| a.alu(Alu::Add, Size::B32, Reg::Rcx, Reg::Rdx)), [0x01, 0xd1]);
    assert_eq!(encoded(|a| a.alu(Alu::Cmp, Size::B8, Reg::Rsi, Reg::Rax)), [0x40, 0x38, 0xc6]);
    assert_eq!(encoded(|a| a.zero_extend(Size::B8, Reg::Rax, Reg::Rdi)), [0x40, 0x0f, 0xb6, 0xc7]);
    assert_eq!(encoded(|a| a.alu_imm(Alu::Sub, Size::B64, Rm::Reg(Reg::Rsp), 16)), [0x48, 0x83, 0xec, 0x10]);
}

#[test]
fn memory_operands_take_sib_and_displacements_where_the_base_needs_them() {
    assert_eq!(encoded(|a| a.load(Size::B64, Reg::Rax, Mem::at(Reg::Rbp, -8))), [0x48, 0x8b, 0x45, 0xf8]);
    assert_eq!(encoded(|a| a.store(Size::B32, Mem::at(Reg::Rsp, 0), Reg::Rcx)), [0x89, 0x0c, 0x24]);
    assert_eq!(encoded(|a| a.load(Size::B8, Reg::Rdx, Mem::at(Reg::R13, 0))), [0x41, 0x0f, 0xb6, 0x55, 0x00]);
    assert_eq!(
        encoded(|a| a.lea(Reg::R11, Mem::at(Reg::Rbp, -512))),
        [0x4c, 0x8d, 0x9d, 0x00, 0xfe, 0xff, 0xff]
    );
}

#[test]
fn immediates_take_the_shortest_move() {
    assert_eq!(encoded(|a| a.mov_ri(Reg::Rax, 1)), [0xb8, 1, 0, 0, 0]);
    assert_eq!(encoded(|a| a.mov_ri(Reg::R8, u64::MAX)), [0x49, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff]);
    assert_eq!(encoded(|a| a.mov_ri(Reg::Rcx, 1 << 40)), [0x48, 0xb9, 0, 0, 0, 0, 0, 1, 0, 0]);
}

/// The code of `f(n: i64) -> i64`, its body built by `build` from `n`.
fn compiled(build: impl FnOnce(&mut Builder, Value, OriginId)) -> Vec<u8> {
    let origins = OriginTable::new();
    let origin = origins.source(Span::new(SourceFiles::new().add("a.vun", "n"), 0, 1));
    let int = AbiType::Int(Scalar::I64, Ext::Sign);
    let signature = Signature { conv: Conv::C, params: vec![int.clone()], result: Some(int) };
    let mut b = Builder::new(Linkage::Export(Symbol::intern("f")), signature, origin);
    let n = b.params(b.current())[0];
    build(&mut b, n, origin);
    let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
    let f = unit.functions.push(b.finish());
    crate::x86_64::func::compile(&unit, &unit.functions[f]).unwrap().code
}

fn to(block: Block) -> Target {
    Target { block, args: Vec::new() }
}

/// `if n < 10 { 1 } else { 2 }`.
fn branching() -> Vec<u8> {
    compiled(|b, n, origin| {
        let ten = b.constant(Scalar::I64, 10, origin);
        let less = b.compare(Cond::SLt, n, ten, origin);
        let (small, large) = (b.block(&[], origin), b.block(&[], origin));
        b.terminate(Terminator::Branch { cond: less, then: to(small), otherwise: to(large) });
        for (block, value) in [(small, 1), (large, 2)] {
            b.switch_to(block);
            let v = b.constant(Scalar::I64, value, origin);
            b.terminate(Terminator::Return(Some(v)));
        }
    })
}

#[test]
fn a_branch_on_a_comparison_jumps_on_its_flags() {
    let code = branching();
    let has = |bytes: [u8; 2]| code.windows(2).any(|w| w == bytes);
    // `jl` on the flags of `cmp`, no `setl` materializing the condition, no `test` of it.
    assert!(has([0x0f, 0x8c]), "{code:02x?}");
    assert!(!has([0x0f, 0x9c]), "{code:02x?}");
}

/// The sum of `0..n`, in a loop whose counter and total are block parameters.
fn summing() -> Vec<u8> {
    compiled(|b, n, origin| {
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
    })
}

#[test]
fn values_live_across_a_loop_take_callee_saved_registers() {
    let code = summing();
    let has = |bytes: &[u8]| code.windows(bytes.len()).any(|w| w == bytes);
    // `rbx` saved below the frame pointer and restored, `mov [rbp + d8], rbx` and back.
    assert!(has(&[0x48, 0x89, 0x5d]), "{code:02x?}");
    assert!(has(&[0x48, 0x8b, 0x5d]), "{code:02x?}");
}
