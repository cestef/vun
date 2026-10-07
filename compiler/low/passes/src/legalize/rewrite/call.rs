//! Calls, wide arguments passed as limbs and a wide result through a slot, and division through
//! the helper of their count of limbs.

use vun_origin::OriginId;

use super::super::Unlegalized;
use super::super::limbs::Limbs;
use super::{Parts, Rewrite};
use vun_lir::{Access, BinOp, CallKind, Callee, InstKind, Value};

impl Rewrite<'_> {
    /// A call, its wide arguments passed as limbs and a wide result through a slot: what the
    /// old call's value becomes, if it has one.
    pub(super) fn call(
        &mut self,
        callee: &Callee,
        args: &[Value],
        kind: CallKind,
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
        let wide_result = signature.returns().and_then(|r| self.legalize.limbs(r)).map(usize::from);
        let callee = match callee {
            Callee::Indirect(v, s) => Callee::Indirect(self.one(*v), Box::new(self.legalize.signature(s)?)),
            other => other.clone(),
        };
        let mut args: Vec<Value> = args.iter().flat_map(|&a| self.flat(a)).collect();
        if let Some(count) = wide_result {
            if kind == CallKind::Tail {
                // The callee writes where the function's caller expects the result.
                args.push(self.result.expect("a function returning a wide integer receives its address"));
                self.b.push(InstKind::Call { callee, args, kind }, None, at);
                return Ok(Some(Parts::Written));
            }
            let limb = self.legalize.limb_bytes();
            let slot = self.b.slot(limb * u64::try_from(count).expect("a few limbs"), limb, at);
            let addr = self.b.slot_addr(slot, at);
            args.push(addr);
            self.b.push(InstKind::Call { callee, args, kind }, None, at);
            return Ok(Some(Parts::Limbs(self.load_limbs(addr, Access::Plain, count, at))));
        }
        let ty = result.map(|v| self.legalize.scalar(self.old.ty(v)));
        Ok(self.b.push(InstKind::Call { callee, args, kind }, ty, at).map(Parts::One))
    }

    /// `a op b` for a division or remainder, through the helper on magnitudes, the signs put
    /// back: a quotient negative where the signs differ, a remainder with the dividend's.
    pub(super) fn divide(&mut self, op: BinOp, dividend: &Limbs, divisor: &Limbs, at: OriginId) -> Limbs {
        let signed = matches!(op, BinOp::SDiv | BinOp::SRem);
        let count = dividend.len();
        let helper = self.helpers.divide(u8::try_from(count).expect("a few limbs"), at);
        let mut l = self.limbwise(at);
        let (negative_a, negative_b) = (l.negative(dividend), l.negative(divisor));
        let (magnitude, divided_by) = if signed {
            let (minus_a, minus_b) = (l.neg(dividend), l.neg(divisor));
            (l.select_limbs(negative_a, &minus_a, dividend), l.select_limbs(negative_b, &minus_b, divisor))
        } else {
            (dividend.clone(), divisor.clone())
        };
        let limb = self.legalize.limb_bytes();
        let words = 2 * count;
        let slot = self.b.slot(limb * u64::try_from(words).expect("a few limbs"), limb, at);
        let addr = self.b.slot_addr(slot, at);
        let mut args: Vec<Value> = magnitude.0.iter().chain(&divided_by.0).copied().collect();
        args.push(addr);
        self.b.push(InstKind::Call { callee: Callee::Func(helper), args, kind: CallKind::Plain }, None, at);
        let mut results = Vec::with_capacity(words);
        for k in 0..words {
            let place = self.past(addr, limb * u64::try_from(k).expect("a few limbs"), at);
            results.push(self.b.load(self.legalize.widest, place, Access::Plain, at));
        }
        let remainder = Limbs(results.split_off(count));
        let quotient = Limbs(results);
        let mut l = self.limbwise(at);
        match op {
            BinOp::UDiv => quotient,
            BinOp::URem => remainder,
            BinOp::SDiv => {
                let flip = l.op(BinOp::Xor, negative_a, negative_b);
                let minus = l.neg(&quotient);
                l.select_limbs(flip, &minus, &quotient)
            }
            _ => {
                let minus = l.neg(&remainder);
                l.select_limbs(negative_a, &minus, &remainder)
            }
        }
    }
}
