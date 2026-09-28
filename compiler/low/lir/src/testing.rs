//! Fixtures for tests of LIR and of what rewrites it (the `testing` feature).

use vun_intern::Symbol;
use vun_origin::{OriginId, OriginTable};
use vun_span::{SourceFiles, Span};

use crate::func::{Conv, Linkage, Signature};
use crate::{AbiType, BinOp, Builder, Cond, Scalar, Target, Terminator, Unit};

/// An origin in a file of its own.
#[must_use]
pub fn origin(origins: &OriginTable) -> OriginId {
    let file = SourceFiles::new().add("a.vun", "sum := (n: u64) -> u64 { ... };");
    origins.source(Span::new(file, 0, 3))
}

/// `sum(n)`: adds `0..n` in a loop whose counter and total are block parameters.
#[must_use]
pub fn sum(origin: OriginId) -> Unit {
    let int = AbiType::Int(Scalar::I64, crate::Ext::Zero);
    let signature = Signature { conv: Conv::C, params: vec![int.clone()], result: Some(int) };
    let mut b = Builder::new(Linkage::Export(Symbol::intern("sum")), signature, origin);
    let n = b.params(b.current())[0];
    let zero = b.constant(Scalar::I64, 0, origin);
    let head = b.block(&[Scalar::I64, Scalar::I64], origin);
    let (body, exit) = (b.block(&[], origin), b.block(&[], origin));
    b.terminate(Terminator::Jump(Target { block: head, args: vec![zero, zero] }));
    b.switch_to(head);
    let [i, total] = [b.params(head)[0], b.params(head)[1]];
    let more = b.compare(Cond::ULt, i, n, origin);
    let to = |block| Target { block, args: Vec::new() };
    b.terminate(Terminator::Branch { cond: more, then: to(body), otherwise: to(exit) });
    b.switch_to(body);
    let one = b.constant(Scalar::I64, 1, origin);
    let (next, added) = (b.binary(BinOp::Add, i, one, origin), b.binary(BinOp::Add, total, i, origin));
    b.terminate(Terminator::Jump(Target { block: head, args: vec![next, added] }));
    b.switch_to(exit);
    b.terminate(Terminator::Return(Some(total)));
    let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
    unit.functions.push(b.finish());
    unit
}
