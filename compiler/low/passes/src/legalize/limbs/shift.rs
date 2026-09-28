//! Shifts of limbs: bits cross between neighbours, and whole limbs move.

use vun_lir::{BinOp, Cond, Value};

use super::{Limbs, Limbwise};

impl Limbwise<'_> {
    /// `a` shifted by `n`, below the wide width: each limb by `n` modulo the limb's width, the
    /// bits crossing from its neighbour moved by a shift in two steps, then whole limbs moved by
    /// the rest of `n`, chosen limb by limb.
    pub(crate) fn shift(&mut self, op: BinOp, a: &Limbs, n: Value) -> Limbs {
        let (count, width) = (a.len(), self.bits());
        let (limit, one) = (self.constant(u128::from(width - 1)), self.constant(1));
        let m = self.op(BinOp::And, n, limit);
        let inverse = self.op(BinOp::Sub, limit, m);
        let log = self.constant(u128::from(width.trailing_zeros()));
        let moved = self.op(BinOp::LShr, n, log);
        // Bits cross from the limb below for a left shift, from the one above for a right one;
        // only the top limb shifts in its sign.
        let left = op == BinOp::Shl;
        let crossing = if left { BinOp::LShr } else { BinOp::Shl };
        let mut within = Vec::with_capacity(count);
        for k in 0..count {
            let top = k + 1 == count;
            let own = self.op(if op == BinOp::AShr && !top { BinOp::LShr } else { op }, a.0[k], m);
            let neighbour = if left { k.checked_sub(1) } else { (!top).then_some(k + 1) };
            within.push(match neighbour {
                Some(j) => {
                    let stepped = self.op(crossing, a.0[j], one);
                    let carried = self.op(crossing, stepped, inverse);
                    self.op(BinOp::Or, own, carried)
                }
                None => own,
            });
        }
        let fill = if op == BinOp::AShr { self.op(BinOp::AShr, a.high(), limit) } else { self.constant(0) };
        let steps: Vec<Value> = (1..count)
            .map(|j| {
                let j = self.constant(u128::try_from(j).expect("a few limbs"));
                self.compare(Cond::Eq, moved, j)
            })
            .collect();
        let zero = self.constant(0);
        let source = |k: usize, j: usize| {
            if left {
                k.checked_sub(j).map_or(zero, |s| within[s])
            } else {
                within.get(k + j).copied().unwrap_or(fill)
            }
        };
        let mut out = Vec::with_capacity(count);
        for k in 0..count {
            let mut v = source(k, 0);
            for (j, &step) in (1..count).zip(&steps) {
                let moved_by = source(k, j);
                v = self.select(step, moved_by, v);
            }
            out.push(v);
        }
        Limbs(out)
    }

    /// `2·a`: each limb shifted left by one, the top bit of the one below into it.
    pub(crate) fn twice(&mut self, a: &Limbs) -> Limbs {
        let (one, top) = (self.constant(1), self.constant(u128::from(self.bits() - 1)));
        let mut out = Vec::with_capacity(a.len());
        for (k, &limb) in a.0.iter().enumerate() {
            let up = self.op(BinOp::Shl, limb, one);
            out.push(match k.checked_sub(1) {
                Some(below) => {
                    let carried = self.op(BinOp::LShr, a.0[below], top);
                    self.op(BinOp::Or, up, carried)
                }
                None => up,
            });
        }
        Limbs(out)
    }
}
