//! `f16` held as its bits, `i16`, for a backend that computes floats no narrower than `f32`:
//! the exact widening to `f32` and the rounding back, to nearest even, as straight-line integer
//! and `f32` operations. An operation on `f16` computes in `f32` between them and rounds once:
//! `f32` holds the exact sum, difference, product or quotient of two `f16`s closely enough
//! (24 bits of significand, at least 2 · 11 + 2) that rounding it again gives the `f16` result.

use vun_origin::OriginId;

use crate::{BinOp, Builder, CastOp, Cond, Scalar, Value};

/// Bits of an `f16`.
const SIGN: u128 = 0x8000;
const FRACTION: u128 = 0x3ff;
const EXPONENT: u128 = 0x1f;
const QUIET_NAN: u128 = 0x7e00;
const INFINITY: u128 = 0x7c00;
/// Where an `f16`'s exponent and fraction sit, and how far they move to an `f32`'s.
const FRACTION_BITS: u128 = 10;
const FRACTION_SHIFT: u128 = 13;
const SIGN_SHIFT: u128 = 16;
/// The `f32` exponent field less the `f16` one: 127 - 15.
const REBIAS: u128 = 112;
/// Bits of an `f32`.
const F32_SIGN: u128 = 0x8000_0000;
const F32_INFINITY: u128 = 0x7f80_0000;
const F32_EXPONENT_SHIFT: u128 = 23;
/// `2^-14`, the smallest normal `f16`, as an `f32`: a subnormal `f16`'s fraction under it,
/// less it, is that subnormal exactly.
const SMALLEST_NORMAL: u128 = (REBIAS + 1) << F32_EXPONENT_SHIFT;
/// The smallest `f32` too large for an `f16`: `2^16`.
const TOO_LARGE: u128 = (127 + 16) << F32_EXPONENT_SHIFT;
/// `0.5` as an `f32`: added to a magnitude under `2^-14`, its rounding to nearest even leaves the
/// `f16` subnormal's bits at the bottom.
const HALF: u128 = 126 << F32_EXPONENT_SHIFT;
/// Rebiases a normal `f32`'s exponent to an `f16`'s and rounds half below the kept bits:
/// `((15 - 127) << 23) + 0xfff`, wrapped to 32 bits.
const REBIAS_DOWN: u128 = 0xc800_0fff;

/// A builder emitting the sequences at `at`.
pub(super) struct Float16<'b> {
    pub(super) b: &'b mut Builder,
    pub(super) at: OriginId,
}

impl Float16<'_> {
    fn int(&mut self, bits: u128) -> Value {
        self.b.constant(Scalar::I32, bits, self.at)
    }

    fn op(&mut self, op: BinOp, a: Value, bits: u128) -> Value {
        let n = self.int(bits);
        self.b.binary(op, a, n, self.at)
    }

    fn select(&mut self, cond: Value, then: Value, otherwise: Value) -> Value {
        self.b.select(cond, then, otherwise, self.at)
    }

    fn float(&mut self, bits: Value) -> Value {
        self.b.cast(CastOp::Bitcast, bits, Scalar::F32, self.at)
    }

    /// The `f32` equal to the `f16` of bits `h`, a NaN's payload kept.
    pub(super) fn widen(&mut self, h: Value) -> Value {
        let h = self.b.cast(CastOp::ZExt, h, Scalar::I32, self.at);
        let sign = self.op(BinOp::And, h, SIGN);
        let sign = self.op(BinOp::Shl, sign, SIGN_SHIFT);
        let field = self.op(BinOp::LShr, h, FRACTION_BITS);
        let field = self.op(BinOp::And, field, EXPONENT);
        let fraction = self.op(BinOp::And, h, FRACTION);
        let fraction = self.op(BinOp::Shl, fraction, FRACTION_SHIFT);
        let signed = self.b.binary(BinOp::Or, sign, fraction, self.at);
        // A normal number: the exponent rebiased.
        let exponent = self.op(BinOp::Add, field, REBIAS);
        let exponent = self.op(BinOp::Shl, exponent, F32_EXPONENT_SHIFT);
        let normal = self.b.binary(BinOp::Or, signed, exponent, self.at);
        // An infinity or a NaN: the exponent all ones.
        let special = self.op(BinOp::Or, signed, F32_INFINITY);
        // A subnormal or a zero: its fraction under the smallest normal, less it, exactly.
        let under = self.op(BinOp::Or, fraction, SMALLEST_NORMAL);
        let (under, smallest) = (self.float(under), self.int(SMALLEST_NORMAL));
        let smallest = self.float(smallest);
        let magnitude = self.b.binary(BinOp::FSub, under, smallest, self.at);
        let magnitude = self.b.cast(CastOp::Bitcast, magnitude, Scalar::I32, self.at);
        let subnormal = self.b.binary(BinOp::Or, magnitude, sign, self.at);
        let (zero, all) = (self.int(0), self.int(EXPONENT));
        let is_subnormal = self.b.compare(Cond::Eq, field, zero, self.at);
        let is_special = self.b.compare(Cond::Eq, field, all, self.at);
        let bits = self.select(is_subnormal, subnormal, normal);
        let bits = self.select(is_special, special, bits);
        self.float(bits)
    }

    /// The bits of the `f16` nearest the `f32` `x`, ties to even; a NaN is a quiet one.
    pub(super) fn narrow(&mut self, x: Value) -> Value {
        let x = self.b.cast(CastOp::Bitcast, x, Scalar::I32, self.at);
        let sign = self.op(BinOp::And, x, F32_SIGN);
        let magnitude = self.b.binary(BinOp::Xor, x, sign, self.at);
        // Too large: an infinity, or a NaN.
        let infinity = self.int(F32_INFINITY);
        let nan = self.b.compare(Cond::UGt, magnitude, infinity, self.at);
        let (quiet, overflowed) = (self.int(QUIET_NAN), self.int(INFINITY));
        let large = self.select(nan, quiet, overflowed);
        // Under the smallest normal: rounded by adding one half, in `f32`.
        let (m, half) = (self.float(magnitude), self.int(HALF));
        let half_f = self.float(half);
        let sum = self.b.binary(BinOp::FAdd, m, half_f, self.at);
        let sum = self.b.cast(CastOp::Bitcast, sum, Scalar::I32, self.at);
        let small = self.b.binary(BinOp::Sub, sum, half, self.at);
        // A normal number: rebiased, rounded to nearest even at bit 13, shifted down.
        let odd = self.op(BinOp::LShr, magnitude, FRACTION_SHIFT);
        let odd = self.op(BinOp::And, odd, 1);
        let rounded = self.op(BinOp::Add, magnitude, REBIAS_DOWN);
        let rounded = self.b.binary(BinOp::Add, rounded, odd, self.at);
        let normal = self.op(BinOp::LShr, rounded, FRACTION_SHIFT);
        let (too_large, smallest) = (self.int(TOO_LARGE), self.int(SMALLEST_NORMAL));
        let is_large = self.b.compare(Cond::UGe, magnitude, too_large, self.at);
        let is_small = self.b.compare(Cond::ULt, magnitude, smallest, self.at);
        let bits = self.select(is_small, small, normal);
        let bits = self.select(is_large, large, bits);
        let sign = self.op(BinOp::LShr, sign, SIGN_SHIFT);
        let bits = self.b.binary(BinOp::Or, bits, sign, self.at);
        self.b.cast(CastOp::Trunc, bits, Scalar::I16, self.at)
    }
}
