//! The division helper: the quotient and remainder of two unsigned wide integers, as pairs, by
//! restoring division one bit at a time, written through the address it receives last as the
//! quotient's halves then the remainder's, low first.

use vun_origin::OriginId;

use super::Legalize;
use super::halves::{Halves, Pair};
use vun_lir::func::{Conv, Linkage};
use vun_lir::{
    AbiType, Access, BinOp, Builder, CastOp, Cond, Ext, Function, Scalar, Signature, Target, Terminator,
    Value,
};

/// `(a, d) -> (a / d, a % d)`, `d` not 0.
pub(super) fn helper(legalize: Legalize, pointer_bits: u32, at: OriginId) -> Function {
    let half = legalize.widest;
    let bytes = legalize.half_bytes();
    let int = AbiType::Int(half, Ext::Zero);
    let signature = Signature {
        conv: Conv::Vun,
        params: vec![int.clone(), int.clone(), int.clone(), int],
        result: Some(AbiType::Memory { size: 4 * bytes, align: bytes }),
        variadic: None,
    };
    let mut build = Builder::new(Linkage::Internal, signature, at);
    let entry = build.params(build.current()).to_vec();
    let (dividend, divisor, out) =
        (Pair { lo: entry[0], hi: entry[1] }, Pair { lo: entry[2], hi: entry[3] }, entry[4]);
    let head = build.block(&[half; 7], at);
    let (body, exit) = (build.block(&[], at), build.block(&[half; 4], at));
    let bits = u128::from(half.bits(0) * 2);
    let to = |block, args: Vec<Value>| Target { block, args };
    let zero = build.constant(half, 0, at);
    build.terminate(Terminator::Jump(to(head, vec![zero, zero, zero, zero, dividend.lo, dividend.hi, zero])));
    build.switch_to(head);
    let state = build.params(head).to_vec();
    let (quotient, remainder, rest, step) = (
        Pair { lo: state[0], hi: state[1] },
        Pair { lo: state[2], hi: state[3] },
        Pair { lo: state[4], hi: state[5] },
        state[6],
    );
    let all = build.constant(half, bits, at);
    let done = build.compare(Cond::Eq, step, all, at);
    build.terminate(Terminator::Branch {
        cond: done,
        then: to(exit, vec![quotient.lo, quotient.hi, remainder.lo, remainder.hi]),
        otherwise: to(body, vec![]),
    });
    build.switch_to(body);
    let mut halves = Halves { b: &mut build, half, at };
    let top = halves.constant(u128::from(half.bits(0) - 1));
    let incoming = halves.op(BinOp::LShr, rest.hi, top);
    let remainder = twice(&mut halves, remainder);
    let remainder = Pair { lo: halves.op(BinOp::Or, remainder.lo, incoming), hi: remainder.hi };
    let (rest, quotient) = (twice(&mut halves, rest), twice(&mut halves, quotient));
    let fits = halves.compared(Cond::UGe, remainder, divisor);
    let less = halves.sub(remainder, divisor);
    let remainder = halves.select_pair(fits, less, remainder);
    let bit = halves.b.cast(CastOp::ZExt, fits, half, at);
    let quotient = Pair { lo: halves.op(BinOp::Or, quotient.lo, bit), hi: quotient.hi };
    let one = halves.constant(1);
    let next = halves.op(BinOp::Add, step, one);
    build.terminate(Terminator::Jump(to(
        head,
        vec![quotient.lo, quotient.hi, remainder.lo, remainder.hi, rest.lo, rest.hi, next],
    )));
    build.switch_to(exit);
    let word = Scalar::int(pointer_bits).expect("addresses are as wide as an integer");
    let results = build.params(exit).to_vec();
    for (k, &value) in results.iter().enumerate() {
        let offset = build.constant(word, u128::from(k as u64 * bytes), at);
        let addr = build.offset(out, offset, at);
        build.store(value, addr, Access::Plain, at);
    }
    build.terminate(Terminator::Return(None));
    build.finish()
}

/// `2·a`: both halves shifted left by one, the low half's top bit into the high half.
fn twice(h: &mut Halves<'_>, a: Pair) -> Pair {
    let (one, top) = (h.constant(1), h.constant(u128::from(h.half.bits(0) - 1)));
    let carried = h.op(BinOp::LShr, a.lo, top);
    let hi = h.op(BinOp::Shl, a.hi, one);
    Pair { lo: h.op(BinOp::Shl, a.lo, one), hi: h.op(BinOp::Or, hi, carried) }
}
