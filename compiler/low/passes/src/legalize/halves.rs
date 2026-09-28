//! Operations on wide integers as pairs of halves, low first, built with a function's builder:
//! the sequences the pass rewrites wide operations into, and the division helper is made of.
//! Every half operation wraps, as LIR's integer operations do.

use vun_codec::{Decode, Encode};
use vun_origin::OriginId;

use vun_lir::{BinOp, Builder, CastOp, Cond, InstKind, Scalar, Value};

/// A wide integer as its halves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub(super) struct Pair {
    pub(super) lo: Value,
    pub(super) hi: Value,
}

/// A builder emitting operations on halves of type `half`.
pub(super) struct Halves<'b> {
    pub(super) b: &'b mut Builder,
    pub(super) half: Scalar,
    pub(super) at: OriginId,
}

impl Halves<'_> {
    fn bits(&self) -> u32 {
        self.half.bits(0)
    }

    pub(super) fn constant(&mut self, bits: u128) -> Value {
        self.b.constant(self.half, bits, self.at)
    }

    /// The wide constant `bits`.
    pub(super) fn pair(&mut self, bits: u128) -> Pair {
        let mask = (1u128 << self.bits()) - 1;
        Pair { lo: self.constant(bits & mask), hi: self.constant(bits >> self.bits() & mask) }
    }

    pub(super) fn op(&mut self, op: BinOp, a: Value, b: Value) -> Value {
        self.b.binary(op, a, b, self.at)
    }

    pub(super) fn compare(&mut self, cond: Cond, a: Value, b: Value) -> Value {
        self.b.compare(cond, a, b, self.at)
    }

    pub(super) fn select(&mut self, cond: Value, then: Value, otherwise: Value) -> Value {
        let ty = self.b.ty(then);
        self.b
            .push(InstKind::Select { cond, then, otherwise }, Some(ty), self.at)
            .expect("a select defines a value")
    }

    /// A condition, `i8`, as a half: 0 or 1.
    fn widened(&mut self, flag: Value) -> Value {
        self.b.cast(CastOp::ZExt, flag, self.half, self.at)
    }

    pub(super) fn bitwise(&mut self, op: BinOp, a: Pair, b: Pair) -> Pair {
        Pair { lo: self.op(op, a.lo, b.lo), hi: self.op(op, a.hi, b.hi) }
    }

    pub(super) fn select_pair(&mut self, cond: Value, then: Pair, otherwise: Pair) -> Pair {
        Pair { lo: self.select(cond, then.lo, otherwise.lo), hi: self.select(cond, then.hi, otherwise.hi) }
    }

    pub(super) fn add(&mut self, a: Pair, b: Pair) -> Pair {
        let lo = self.op(BinOp::Add, a.lo, b.lo);
        let carry = self.compare(Cond::ULt, lo, a.lo);
        let carry = self.widened(carry);
        let hi = self.op(BinOp::Add, a.hi, b.hi);
        Pair { lo, hi: self.op(BinOp::Add, hi, carry) }
    }

    pub(super) fn sub(&mut self, a: Pair, b: Pair) -> Pair {
        let borrow = self.compare(Cond::ULt, a.lo, b.lo);
        let borrow = self.widened(borrow);
        let lo = self.op(BinOp::Sub, a.lo, b.lo);
        let hi = self.op(BinOp::Sub, a.hi, b.hi);
        Pair { lo, hi: self.op(BinOp::Sub, hi, borrow) }
    }

    /// `0 - a`.
    pub(super) fn neg(&mut self, a: Pair) -> Pair {
        let zero = self.pair(0);
        self.sub(zero, a)
    }

    /// The full product of two halves, through quarters so that no partial product overflows.
    fn wide_product(&mut self, x: Value, y: Value) -> Pair {
        let quarter = self.bits() / 2;
        let (mask, shift) = (self.constant((1 << quarter) - 1), self.constant(u128::from(quarter)));
        let parts = |h: &mut Self, v: Value| (h.op(BinOp::And, v, mask), h.op(BinOp::LShr, v, shift));
        let ((x0, x1), (y0, y1)) = (parts(self, x), parts(self, y));
        let (p00, p01) = (self.op(BinOp::Mul, x0, y0), self.op(BinOp::Mul, x0, y1));
        let (p10, p11) = (self.op(BinOp::Mul, x1, y0), self.op(BinOp::Mul, x1, y1));
        let (p00_hi, p01_lo, p10_lo) = (
            self.op(BinOp::LShr, p00, shift),
            self.op(BinOp::And, p01, mask),
            self.op(BinOp::And, p10, mask),
        );
        let mid = self.op(BinOp::Add, p00_hi, p01_lo);
        let mid = self.op(BinOp::Add, mid, p10_lo);
        let (p00_lo, mid_up) = (self.op(BinOp::And, p00, mask), self.op(BinOp::Shl, mid, shift));
        let lo = self.op(BinOp::Or, p00_lo, mid_up);
        let (p01_hi, p10_hi, mid_hi) = (
            self.op(BinOp::LShr, p01, shift),
            self.op(BinOp::LShr, p10, shift),
            self.op(BinOp::LShr, mid, shift),
        );
        let hi = self.op(BinOp::Add, p11, p01_hi);
        let hi = self.op(BinOp::Add, hi, p10_hi);
        Pair { lo, hi: self.op(BinOp::Add, hi, mid_hi) }
    }

    /// The low half of the product's width: the wide product wraps.
    pub(super) fn mul(&mut self, a: Pair, b: Pair) -> Pair {
        let low = self.wide_product(a.lo, b.lo);
        let (cross, other) = (self.op(BinOp::Mul, a.lo, b.hi), self.op(BinOp::Mul, a.hi, b.lo));
        let hi = self.op(BinOp::Add, low.hi, cross);
        Pair { lo: low.lo, hi: self.op(BinOp::Add, hi, other) }
    }

    /// `a` shifted by `n`, below the wide width: each half by `n` modulo the half's width, the
    /// bits crossing between halves moved by a shift in two steps, and the halves exchanged
    /// from the half's width on.
    pub(super) fn shift(&mut self, op: BinOp, a: Pair, n: Value) -> Pair {
        let top = u128::from(self.bits() - 1);
        let (limit, one) = (self.constant(top), self.constant(1));
        let m = self.op(BinOp::And, n, limit);
        let inverse = self.op(BinOp::Sub, limit, m);
        let width = self.constant(u128::from(self.bits()));
        let big = self.compare(Cond::UGe, n, width);
        let zero = self.constant(0);
        if op == BinOp::Shl {
            {
                let lo = self.op(BinOp::Shl, a.lo, m);
                let half = self.op(BinOp::LShr, a.lo, one);
                let carried = self.op(BinOp::LShr, half, inverse);
                let hi = self.op(BinOp::Shl, a.hi, m);
                let hi = self.op(BinOp::Or, hi, carried);
                Pair { lo: self.select(big, zero, lo), hi: self.select(big, lo, hi) }
            }
        } else {
            {
                let hi = self.op(op, a.hi, m);
                let twice = self.op(BinOp::Shl, a.hi, one);
                let carried = self.op(BinOp::Shl, twice, inverse);
                let lo = self.op(BinOp::LShr, a.lo, m);
                let lo = self.op(BinOp::Or, lo, carried);
                let fill = if op == BinOp::AShr { self.op(BinOp::AShr, a.hi, limit) } else { zero };
                Pair { lo: self.select(big, hi, lo), hi: self.select(big, fill, hi) }
            }
        }
    }

    /// A comparison of wide integers: the high halves decide, as `cond` reads them, unless
    /// equal, then the low halves, unsigned.
    pub(super) fn compared(&mut self, cond: Cond, a: Pair, b: Pair) -> Value {
        match cond {
            Cond::Eq | Cond::Ne => {
                let (lo, hi) = (self.compare(cond, a.lo, b.lo), self.compare(cond, a.hi, b.hi));
                self.op(if cond == Cond::Eq { BinOp::And } else { BinOp::Or }, lo, hi)
            }
            _ => {
                let strict = match cond {
                    Cond::SLe => Cond::SLt,
                    Cond::SGe => Cond::SGt,
                    Cond::ULe => Cond::ULt,
                    Cond::UGe => Cond::UGt,
                    other => other,
                };
                let high = self.compare(strict, a.hi, b.hi);
                let equal = self.compare(Cond::Eq, a.hi, b.hi);
                let low = self.compare(cond.unsigned(), a.lo, b.lo);
                self.select(equal, low, high)
            }
        }
    }

    /// `v`, an integer no wider than a half, extended to a pair.
    pub(super) fn extended(&mut self, v: Value, signed: bool) -> Pair {
        let lo = if self.b.ty(v) == self.half {
            v
        } else {
            self.b.cast(if signed { CastOp::SExt } else { CastOp::ZExt }, v, self.half, self.at)
        };
        let hi = if signed {
            let top = self.constant(u128::from(self.bits() - 1));
            self.op(BinOp::AShr, lo, top)
        } else {
            self.constant(0)
        };
        Pair { lo, hi }
    }

    /// Whether `a`, signed, is negative.
    pub(super) fn negative(&mut self, a: Pair) -> Value {
        let zero = self.constant(0);
        self.compare(Cond::SLt, a.hi, zero)
    }
}
