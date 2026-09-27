//! Tests of the wasm backend: modules the verifier accepts are ones an independent validator
//! accepts, and the verifier rejects what the specification does.

use vun_intern::Symbol;

use crate::module::op::{Num, Op};
use crate::module::{Binding, Def, Func, FuncType, Invalid, Kind, Linker, Module, Sym, ValType, Why};

/// A module of one exported function of `ty` with `body`, the stack pointer and table named.
fn module(ty: FuncType, locals: Vec<ValType>, body: Vec<Op>) -> Module {
    let mut m = Module::default();
    m.linked(Linker::StackPointer);
    m.linked(Linker::Table);
    let ty = m.ty(ty);
    let kind = Kind::Func { ty, def: Def::Here(0) };
    m.symbols.push(Sym { name: Symbol::intern("f"), kind, binding: Binding::Exported });
    m.funcs.push(Func { ty, locals, body });
    m
}

fn binary(result: ValType) -> FuncType {
    FuncType { params: vec![ValType::I32, ValType::I32], results: vec![result] }
}

/// Validates `bytes` as wasmparser does, with the features the backend uses.
fn validate(bytes: &[u8]) -> Result<(), wasmparser::BinaryReaderError> {
    wasmparser::Validator::new().validate_all(bytes).map(drop)
}

#[test]
fn a_verified_module_is_valid() {
    let sp = Module::default().linked(Linker::StackPointer);
    let body = vec![
        Op::GlobalGet(sp),
        Op::Drop,
        Op::LocalGet(0),
        Op::LocalGet(1),
        Op::Num(Num::I32Add),
        Op::LocalTee(2),
        Op::If,
        Op::LocalGet(2),
        Op::Return,
        Op::Else,
        Op::I32Const(0),
        Op::Return,
        Op::End,
        Op::Unreachable,
    ];
    let m = module(binary(ValType::I32), vec![ValType::I32], body);
    assert_eq!(m.verify(), Ok(()));
    validate(&m.write()).unwrap();
}

#[test]
fn a_loop_and_a_table_verify_and_validate() {
    let body = vec![
        Op::Block,
        Op::Loop,
        Op::LocalGet(0),
        Op::BrTable { cases: vec![0, 1], default: 1 },
        Op::End,
        Op::End,
        Op::I64Const(7),
    ];
    let m = module(binary(ValType::I64), Vec::new(), body);
    assert_eq!(m.verify(), Ok(()));
    validate(&m.write()).unwrap();
}

#[test]
fn an_operand_of_another_type_is_rejected() {
    let body = vec![Op::LocalGet(0), Op::I64Const(1), Op::Num(Num::I32Add)];
    let m = module(binary(ValType::I32), Vec::new(), body);
    let why = Why::Mismatch { expected: ValType::I32, found: ValType::I64 };
    assert_eq!(m.verify(), Err(Invalid { func: 0, at: 2, why }));
}

#[test]
fn a_block_left_holding_an_operand_is_rejected() {
    let body = vec![Op::Block, Op::I32Const(1), Op::End, Op::I32Const(0)];
    let m = module(binary(ValType::I32), Vec::new(), body);
    assert_eq!(m.verify(), Err(Invalid { func: 0, at: 2, why: Why::Leftover }));
}

#[test]
fn a_branch_past_every_block_is_rejected() {
    let body = vec![Op::Br(3)];
    let m = module(binary(ValType::I32), Vec::new(), body);
    assert_eq!(m.verify(), Err(Invalid { func: 0, at: 0, why: Why::Label(3) }));
}

#[test]
fn a_function_ending_without_its_result_is_rejected() {
    let m = module(binary(ValType::I32), Vec::new(), Vec::new());
    assert_eq!(m.verify(), Err(Invalid { func: 0, at: 0, why: Why::Underflow }));
}
