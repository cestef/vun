//! The division helper of integers of a count of limbs: the quotient and remainder of two
//! unsigned wide integers by restoring division one bit at a time, written through the address it
//! receives last as the quotient's limbs then the remainder's, low first.

use vun_origin::OriginId;

use super::Legalize;
use super::limbs::{Limbs, Limbwise};
use vun_lir::func::{Conv, Linkage};
use vun_lir::{
    AbiType, Access, BinOp, Builder, CastOp, Cond, Ext, Function, Scalar, Signature, Target, Terminator,
    Value,
};

/// `(a, d) -> (a / d, a % d)` of `count` limbs each, `d` not 0.
pub(super) fn helper(legalize: Legalize, count: u8, pointer_bits: u32, at: OriginId) -> Function {
    let (limb, n) = (legalize.widest, usize::from(count));
    let bytes = legalize.limb_bytes();
    let int = AbiType::Int(limb, Ext::Zero);
    let signature = Signature {
        conv: Conv::Vun,
        params: vec![int; 2 * n],
        result: Some(AbiType::Memory { size: 2 * u64::from(count) * bytes, align: bytes }),
        variadic: None,
    };
    let mut build = Builder::new(Linkage::Internal, signature, at);
    let entry = build.params(build.current()).to_vec();
    let (dividend, divisor, out) =
        (Limbs(entry[..n].to_vec()), Limbs(entry[n..2 * n].to_vec()), entry[2 * n]);
    // The loop's state: the quotient, the remainder, the dividend's bits not yet read, and the
    // step.
    let head = build.block(&vec![limb; 3 * n + 1], at);
    let (body, exit) = (build.block(&[], at), build.block(&vec![limb; 2 * n], at));
    let bits = u128::from(limb.bits(0)) * u128::from(count);
    let to = |block, args: Vec<Value>| Target { block, args };
    let zero = build.constant(limb, 0, at);
    let start: Vec<Value> = [vec![zero; 2 * n], dividend.0, vec![zero]].concat();
    build.terminate(Terminator::Jump(to(head, start)));
    build.switch_to(head);
    let state = build.params(head).to_vec();
    let quotient = Limbs(state[..n].to_vec());
    let remainder = Limbs(state[n..2 * n].to_vec());
    let rest = Limbs(state[2 * n..3 * n].to_vec());
    let step = state[3 * n];
    let all = build.constant(limb, bits, at);
    let done = build.compare(Cond::Eq, step, all, at);
    build.terminate(Terminator::Branch {
        cond: done,
        then: to(exit, [quotient.0.clone(), remainder.0.clone()].concat()),
        otherwise: to(body, vec![]),
    });
    build.switch_to(body);
    let mut l = Limbwise { b: &mut build, limb, at };
    let top = l.constant(u128::from(limb.bits(0) - 1));
    let incoming = l.op(BinOp::LShr, rest.high(), top);
    let mut remainder = l.twice(&remainder);
    remainder.0[0] = l.op(BinOp::Or, remainder.low(), incoming);
    let (rest, mut quotient) = (l.twice(&rest), l.twice(&quotient));
    let fits = l.compared(Cond::UGe, &remainder, &divisor);
    let less = l.sub(&remainder, &divisor);
    let remainder = l.select_limbs(fits, &less, &remainder);
    let bit = l.b.cast(CastOp::ZExt, fits, limb, at);
    quotient.0[0] = l.op(BinOp::Or, quotient.low(), bit);
    let one = l.constant(1);
    let next = l.op(BinOp::Add, step, one);
    let again: Vec<Value> = [quotient.0, remainder.0, rest.0, vec![next]].concat();
    build.terminate(Terminator::Jump(to(head, again)));
    build.switch_to(exit);
    let word = Scalar::int(pointer_bits).expect("addresses are as wide as an integer");
    let results = build.params(exit).to_vec();
    for (k, &value) in (0u64..).zip(&results) {
        let offset = build.constant(word, u128::from(k * bytes), at);
        let addr = build.offset(out, offset, at);
        build.store(value, addr, Access::Plain, at);
    }
    build.terminate(Terminator::Return(None));
    build.finish()
}
