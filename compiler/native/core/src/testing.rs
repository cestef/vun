//! Fixtures for tests of each architecture's instruction compilers (the `testing` feature):
//! functions built, compiled through the framework and listed.
#![expect(
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    reason = "fixtures of tests, which panic on what they do not expect"
)]

use std::fmt::Display;

use vun_index::Idx;
use vun_intern::Symbol;
use vun_lir::func::{Conv, Linkage, Signature};
use vun_lir::{
    AbiType, BinOp, Block, Builder, Callee, Cond, Ext, FuncId, InstKind, Scalar, Target, Terminator, Unit,
    Value,
};
use vun_origin::{OriginId, OriginTable};
use vun_span::{SourceFiles, Span};

use vun_codegen::Processor;

use crate::{Code, Native};

/// `f(n: i64) -> i64` for `A`, its body built by `build` from `n`, compiled and listed.
pub fn compiled<A: Native>(build: impl FnOnce(&mut Builder, Value, OriginId)) -> Code<A> {
    compile::<A>(build).unwrap()
}

/// `f(n: i64) -> i64` compiled for a processor having everything, or what the backend lacks for
/// it.
pub fn compile<A: Native>(
    build: impl FnOnce(&mut Builder, Value, OriginId),
) -> Result<Code<A>, vun_backend::Lack> {
    compile_for::<A>(Processor::<A>::every(), Scalar::I64, build)
}

/// `f(n: int) -> int` for `processor`, `int` the integer scalar `int`, compiled and listed, or
/// what the backend or the processor lacks for it.
pub fn compile_for<A: Native>(
    processor: Processor<A>,
    int: Scalar,
    build: impl FnOnce(&mut Builder, Value, OriginId),
) -> Result<Code<A>, vun_backend::Lack> {
    let origins = OriginTable::new();
    let origin = origins.source(Span::new(SourceFiles::new().add("a.vun", "n"), 0, 1));
    let int = AbiType::Int(int, Ext::Sign);
    let signature = Signature {
        conv: Conv::C,
        params: vec![int.clone()],
        result: Some(int),
        variadic: None,
        convention: None,
    };
    let mut b = Builder::new(Linkage::Export(Symbol::intern("f")), signature, origin);
    let n = b.params(b.current())[0];
    build(&mut b, n, origin);
    let pointer_bits = u32::try_from(A::WORD * 8).expect("a word's bits fit");
    let mut unit = Unit { pointer_bits, ..Unit::default() };
    let f = unit.functions.push(b.finish());
    // The first convention the architecture follows.
    vun_codegen::compile::<A>(&unit, &unit.functions[f], A::CONVENTIONS[0].1, processor, true)
}

/// The sum of `0..n`, in a loop whose counter and total are block parameters, the total passed
/// through a call of the function itself each round when `calls`.
pub fn sum(b: &mut Builder, n: Value, origin: OriginId, calls: bool) {
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
    let next = b.binary(BinOp::Add, i, one, origin);
    let mut added = b.binary(BinOp::Add, total, i, origin);
    if calls {
        let call = InstKind::Call { callee: Callee::Func(FuncId::new(0)), args: vec![added] };
        added = b.push(call, Some(Scalar::I64), origin).expect("a call with a result");
    }
    b.terminate(Terminator::Jump(Target { block: head, args: vec![next, added] }));
    b.switch_to(exit);
    b.terminate(Terminator::Return(Some(total)));
}

/// The instructions of `code`, as the listing prints them.
pub fn text<T: Display>(code: &vun_backend::Compiled<T>) -> Vec<String> {
    code.code
        .listing
        .iter()
        .filter_map(|line| match line {
            vun_asm::Line::Inst { inst, .. } => Some(inst.to_string()),
            vun_asm::Line::Label(_) | vun_asm::Line::Word { .. } => None,
        })
        .collect()
}

pub fn to(block: Block) -> Target {
    Target { block, args: Vec::new() }
}
