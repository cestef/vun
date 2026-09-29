//! Operations on wide integers as limbs, low first, built with a function's builder: the
//! sequences the pass rewrites wide operations into, and the division helper is made of. Every
//! limb operation wraps, as LIR's integer operations do; carries and borrows run from the low
//! limb up.

use vun_codec::{Decode, Encode};
use vun_origin::OriginId;

mod arith;
mod shift;

use vun_lir::{BinOp, Builder, CastOp, Cond, InstKind, Scalar, Value};

/// A wide integer as its limbs, low first.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub(crate) struct Limbs(pub(crate) Vec<Value>);

impl Limbs {
    pub(crate) fn low(&self) -> Value {
        self.0[0]
    }

    pub(crate) fn high(&self) -> Value {
        *self.0.last().expect("a wide integer has limbs")
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }
}

/// A builder emitting operations on limbs of type `limb`.
pub(crate) struct Limbwise<'b> {
    pub(crate) b: &'b mut Builder,
    pub(crate) limb: Scalar,
    pub(crate) at: OriginId,
}

impl Limbwise<'_> {
    fn bits(&self) -> u32 {
        self.limb.bits(0)
    }

    pub(crate) fn constant(&mut self, bits: u128) -> Value {
        self.b.constant(self.limb, bits, self.at)
    }

    /// The wide constant `bits` in `count` limbs.
    pub(crate) fn limbs(&mut self, bits: u128, count: usize) -> Limbs {
        let (width, mask) = (self.bits(), u128::MAX >> (u128::BITS - self.bits()));
        let shifts = (0..count).map(|k| u32::try_from(k).expect("a few limbs") * width);
        Limbs(shifts.map(|shift| self.constant(bits.checked_shr(shift).unwrap_or(0) & mask)).collect())
    }

    pub(crate) fn op(&mut self, op: BinOp, a: Value, b: Value) -> Value {
        self.b.binary(op, a, b, self.at)
    }

    pub(crate) fn compare(&mut self, cond: Cond, a: Value, b: Value) -> Value {
        self.b.compare(cond, a, b, self.at)
    }

    pub(crate) fn select(&mut self, cond: Value, then: Value, otherwise: Value) -> Value {
        let ty = self.b.ty(then);
        self.b
            .push(InstKind::Select { cond, then, otherwise }, Some(ty), self.at)
            .expect("a select defines a value")
    }

    /// A condition, `i8`, as a limb: 0 or 1.
    pub(crate) fn widened(&mut self, flag: Value) -> Value {
        self.b.cast(CastOp::ZExt, flag, self.limb, self.at)
    }

    /// 1 where `a` is below `b`, unsigned, else 0, as a limb: the carry of a sum `a` that
    /// wrapped past its operand `b`, or the borrow of `a - b`.
    fn below(&mut self, a: Value, b: Value) -> Value {
        let flag = self.compare(Cond::ULt, a, b);
        self.widened(flag)
    }

    pub(crate) fn bitwise(&mut self, op: BinOp, a: &Limbs, b: &Limbs) -> Limbs {
        Limbs(a.0.iter().zip(&b.0).map(|(&x, &y)| self.op(op, x, y)).collect())
    }

    pub(crate) fn select_limbs(&mut self, cond: Value, then: &Limbs, otherwise: &Limbs) -> Limbs {
        Limbs(then.0.iter().zip(&otherwise.0).map(|(&t, &o)| self.select(cond, t, o)).collect())
    }

    /// A comparison of wide integers: the highest limbs that differ decide, the top one as
    /// `cond` reads it, the others unsigned.
    pub(crate) fn compared(&mut self, cond: Cond, a: &Limbs, b: &Limbs) -> Value {
        match cond {
            Cond::Eq | Cond::Ne => {
                let join = if cond == Cond::Eq { BinOp::And } else { BinOp::Or };
                let each: Vec<Value> =
                    a.0.iter().zip(&b.0).map(|(&x, &y)| self.compare(cond, x, y)).collect();
                each.into_iter().reduce(|r, c| self.op(join, r, c)).expect("a wide integer has limbs")
            }
            _ => {
                let strict = match cond {
                    Cond::SLe => Cond::SLt,
                    Cond::SGe => Cond::SGt,
                    Cond::ULe => Cond::ULt,
                    Cond::UGe => Cond::UGt,
                    other => other,
                };
                let mut decided = self.compare(cond.unsigned(), a.low(), b.low());
                for k in 1..a.len() {
                    let here = if k + 1 == a.len() { strict } else { strict.unsigned() };
                    let (x, y) = (a.0[k], b.0[k]);
                    let (differs, equal) = (self.compare(here, x, y), self.compare(Cond::Eq, x, y));
                    decided = self.select(equal, decided, differs);
                }
                decided
            }
        }
    }

    /// The limbs above `top`, the highest limb of an integer: copies of its sign bit, or zeros.
    fn fill(&mut self, top: Value, signed: bool) -> Value {
        if signed {
            let last = self.constant(u128::from(self.bits() - 1));
            self.op(BinOp::AShr, top, last)
        } else {
            self.constant(0)
        }
    }

    /// `v`, an integer no wider than a limb, extended to `count` limbs.
    pub(crate) fn extended(&mut self, v: Value, signed: bool, count: usize) -> Limbs {
        let low = if self.b.ty(v) == self.limb {
            v
        } else {
            self.b.cast(if signed { CastOp::SExt } else { CastOp::ZExt }, v, self.limb, self.at)
        };
        if count == 1 {
            return Limbs(vec![low]);
        }
        let fill = self.fill(low, signed);
        Limbs(std::iter::once(low).chain(std::iter::repeat_n(fill, count - 1)).collect())
    }

    /// `a` as `count` limbs: its low ones, or all of them extended.
    pub(crate) fn resized(&mut self, a: &Limbs, signed: bool, count: usize) -> Limbs {
        if count <= a.len() {
            return Limbs(a.0[..count].to_vec());
        }
        let fill = self.fill(a.high(), signed);
        Limbs(a.0.iter().copied().chain(std::iter::repeat_n(fill, count - a.len())).collect())
    }

    /// Whether `a`, signed, is negative.
    pub(crate) fn negative(&mut self, a: &Limbs) -> Value {
        let zero = self.constant(0);
        self.compare(Cond::SLt, a.high(), zero)
    }
}
