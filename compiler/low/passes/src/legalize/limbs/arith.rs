//! Sums, differences and products of limbs: carries and borrows run from the low limb up, and a
//! product of two limbs is made whole through halves of them.

use vun_lir::{BinOp, Value};

use super::{Limbs, Limbwise};

impl Limbwise<'_> {
    /// Adds `term` into `acc` from limb `from` up, the carry running to the top limb; a limb of
    /// `acc` that is `None` is zero.
    pub(crate) fn add_into(&mut self, acc: &mut [Option<Value>], from: usize, term: &[Value]) {
        let mut carry = None;
        for k in from..acc.len() {
            let y = term.get(k - from).copied();
            if y.is_none() && carry.is_none() {
                break;
            }
            let last = k + 1 == acc.len();
            let (sum, first) = match (acc[k], y) {
                (Some(x), Some(y)) => {
                    let sum = self.op(BinOp::Add, x, y);
                    (Some(sum), (!last).then(|| self.below(sum, x)))
                }
                (x, y) => (x.or(y), None),
            };
            let (total, second) = match (sum, carry) {
                (Some(sum), Some(c)) => {
                    let total = self.op(BinOp::Add, sum, c);
                    (total, (!last).then(|| self.below(total, sum)))
                }
                (sum, c) => (sum.or(c).expect("a limb or a carry is added"), None),
            };
            acc[k] = Some(total);
            carry = match (first, second) {
                (Some(a), Some(b)) => Some(self.op(BinOp::Or, a, b)),
                (a, b) => a.or(b),
            };
        }
    }

    /// `acc` with its zero limbs made.
    pub(crate) fn made(&mut self, acc: Vec<Option<Value>>) -> Limbs {
        Limbs(acc.into_iter().map(|v| v.unwrap_or_else(|| self.constant(0))).collect())
    }

    pub(crate) fn add(&mut self, a: &Limbs, b: &Limbs) -> Limbs {
        let mut acc: Vec<Option<Value>> = a.0.iter().copied().map(Some).collect();
        self.add_into(&mut acc, 0, &b.0);
        self.made(acc)
    }

    pub(crate) fn sub(&mut self, a: &Limbs, b: &Limbs) -> Limbs {
        let mut borrow: Option<Value> = None;
        let mut out = Vec::with_capacity(a.len());
        for (k, (&x, &y)) in a.0.iter().zip(&b.0).enumerate() {
            let last = k + 1 == a.len();
            let diff = self.op(BinOp::Sub, x, y);
            let first = (!last).then(|| self.below(x, y));
            let (total, second) = match borrow {
                Some(b) => {
                    let total = self.op(BinOp::Sub, diff, b);
                    (total, (!last).then(|| self.below(diff, b)))
                }
                None => (diff, None),
            };
            out.push(total);
            borrow = match (first, second) {
                (Some(a), Some(b)) => Some(self.op(BinOp::Or, a, b)),
                (a, b) => a.or(b),
            };
        }
        Limbs(out)
    }

    /// `0 - a`.
    pub(crate) fn neg(&mut self, a: &Limbs) -> Limbs {
        let zero = self.limbs(0, a.len());
        self.sub(&zero, a)
    }

    /// The full product of two limbs, low then high, through halves of them so that no partial
    /// product overflows.
    pub(crate) fn wide_product(&mut self, x: Value, y: Value) -> [Value; 2] {
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
        [lo, self.op(BinOp::Add, hi, mid_hi)]
    }

    /// The low limbs of the product, which wraps: each product of two limbs added at the limb
    /// of their positions' sum, whole where it reaches no further than the top limb.
    pub(crate) fn mul(&mut self, a: &Limbs, b: &Limbs) -> Limbs {
        let count = a.len();
        let mut acc = vec![None; count];
        for (i, &x) in a.0.iter().enumerate() {
            for (j, &y) in b.0.iter().take(count - i).enumerate() {
                let term = if i + j + 1 == count {
                    vec![self.op(BinOp::Mul, x, y)]
                } else {
                    self.wide_product(x, y).to_vec()
                };
                self.add_into(&mut acc, i + j, &term);
            }
        }
        self.made(acc)
    }
}
