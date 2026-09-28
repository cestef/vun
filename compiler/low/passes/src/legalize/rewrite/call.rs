//! Calls, wide arguments passed as halves and a wide result through a slot, and division through
//! the helper.

use vun_origin::OriginId;

use super::super::Unlegalized;
use super::super::halves::Pair;
use super::{Parts, Rewrite};
use vun_lir::{Access, BinOp, Callee, InstKind, Value};

impl Rewrite<'_> {
    /// A call, its wide arguments passed as halves and a wide result through a slot: what the
    /// old call's value becomes, if it has one.
    pub(super) fn call(
        &mut self,
        callee: &Callee,
        args: &[Value],
        result: Option<Value>,
        at: OriginId,
    ) -> Result<Option<Parts>, Unlegalized> {
        let signature = match callee {
            Callee::Func(f) => &self.unit.functions[*f].signature,
            Callee::Import(i) => match &self.unit.imports[*i].kind {
                vun_lir::ImportKind::Func(s) => s,
                vun_lir::ImportKind::Data => unreachable!("the verifier checks calls call functions"),
            },
            Callee::Indirect(_, s) => s,
        };
        let wide_result = signature.returns().is_some_and(|r| Some(r) == self.wide);
        let callee = match callee {
            Callee::Indirect(v, s) => {
                Callee::Indirect(self.one(*v), Box::new(self.legalize.signature(s, self.wide)?))
            }
            other => other.clone(),
        };
        let mut args: Vec<Value> = args.iter().flat_map(|&a| self.flat(a)).collect();
        if wide_result {
            let bytes = 2 * self.legalize.half_bytes();
            let slot = self.b.slot(bytes, self.legalize.half_bytes(), at);
            let addr = self.b.slot_addr(slot, at);
            args.push(addr);
            self.b.push(InstKind::Call { callee, args }, None, at);
            return Ok(Some(Parts::Pair(self.load_pair(addr, Access::Plain, at))));
        }
        let ty = result.map(|v| self.legalize.scalar(self.old.ty(v)));
        Ok(self.b.push(InstKind::Call { callee, args }, ty, at).map(Parts::One))
    }

    /// `a op b` for a division or remainder, through the helper on magnitudes, the signs put
    /// back: a quotient negative where the signs differ, a remainder with the dividend's.
    pub(super) fn divide(&mut self, op: BinOp, dividend: Pair, divisor: Pair, at: OriginId) -> Pair {
        let signed = matches!(op, BinOp::SDiv | BinOp::SRem);
        let helper = self.helpers.divide(at);
        let mut halves = self.halves(at);
        let (negative_a, negative_b) = (halves.negative(dividend), halves.negative(divisor));
        let (magnitude, divided_by) = if signed {
            let (minus_a, minus_b) = (halves.neg(dividend), halves.neg(divisor));
            (
                halves.select_pair(negative_a, minus_a, dividend),
                halves.select_pair(negative_b, minus_b, divisor),
            )
        } else {
            (dividend, divisor)
        };
        let bytes = self.legalize.half_bytes();
        let slot = self.b.slot(4 * bytes, bytes, at);
        let addr = self.b.slot_addr(slot, at);
        self.b.push(
            InstKind::Call {
                callee: Callee::Func(helper),
                args: vec![magnitude.lo, magnitude.hi, divided_by.lo, divided_by.hi, addr],
            },
            None,
            at,
        );
        let mut words = Vec::new();
        for k in 0..4 {
            let at_k = self.past(addr, k * bytes, at);
            words.push(self.b.load(self.legalize.widest, at_k, Access::Plain, at));
        }
        let (quotient, remainder) =
            (Pair { lo: words[0], hi: words[1] }, Pair { lo: words[2], hi: words[3] });
        let mut halves = self.halves(at);
        match op {
            BinOp::UDiv => quotient,
            BinOp::URem => remainder,
            BinOp::SDiv => {
                let flip = halves.op(BinOp::Xor, negative_a, negative_b);
                let minus = halves.neg(quotient);
                halves.select_pair(flip, minus, quotient)
            }
            _ => {
                let minus = halves.neg(remainder);
                halves.select_pair(negative_a, minus, remainder)
            }
        }
    }
}
