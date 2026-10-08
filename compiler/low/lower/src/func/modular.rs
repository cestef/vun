//! Reduction modulo a size whose offsets do not fit 128 bits (spec 04, rule 9).

use vun_lir::{BinOp, Cond, Scalar, Value};
use vun_sema_int::{Integer, Interval};

use super::FnLower;
use crate::repr::Types;
use crate::unlowered::{Form, Unlowered};

impl FnLower<'_, '_, '_> {
    /// `x`, held in `source` with values of `own`, reduced modulo `size` into the range starting
    /// at `lo`, held in `target`: the value moved into `[0, size)`, then its offset from `lo`,
    /// both in a storage holding `own` and the size, or in 128 unsigned bits where a negative
    /// value is raised by a size above `2^127`, then added to `lo` in `target`.
    pub(super) fn wrap_wide(
        &mut self,
        x: Value,
        (source, signed, own): (Scalar, bool, &Interval),
        (lo, size): (&Integer, &Integer),
        target: Scalar,
    ) -> Result<Value, Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Conversion);
        let reach = Interval::closed(Integer::zero(), size.clone()).ok_or_else(unsupported)?;
        let lo_offset = lo.modulo(size).and_then(|n| n.to_bits()).ok_or_else(unsupported)?;
        let low = lo.to_bits().ok_or_else(unsupported)?;
        let modulus = size.to_bits().ok_or_else(unsupported)?;
        let (work, held) = if let Some((work, work_signed)) = Types::storing(&own.hull(&reach)) {
            let x = self.resize(x, source, signed, work);
            let modulus = self.b.constant(work, work.truncate(modulus), at);
            let op = if work_signed { BinOp::SRem } else { BinOp::URem };
            let mut held = self.b.binary(op, x, modulus, at);
            if work_signed {
                let zero = self.b.constant(work, 0, at);
                let below = self.b.compare(Cond::SLt, held, zero, at);
                let moved = self.b.binary(BinOp::Add, held, modulus, at);
                held = self.b.select(below, moved, held, at);
            }
            (work, held)
        } else {
            let work = Scalar::int(128).filter(|_| signed).ok_or_else(unsupported)?;
            let zero = self.b.constant(source, 0, at);
            let below = self.b.compare(Cond::SLt, x, zero, at);
            let x = self.resize(x, source, true, work);
            let modulus = self.b.constant(work, work.truncate(modulus), at);
            let moved = self.b.binary(BinOp::Add, x, modulus, at);
            (work, self.b.select(below, moved, x, at))
        };
        let modulus = self.b.constant(work, work.truncate(modulus), at);
        let start = self.b.constant(work, work.truncate(lo_offset), at);
        let offset = self.b.binary(BinOp::Sub, held, start, at);
        let before = self.b.compare(Cond::ULt, held, start, at);
        let raised = self.b.binary(BinOp::Add, offset, modulus, at);
        let reduced = self.b.select(before, raised, offset, at);
        let reduced = self.resize(reduced, work, false, target);
        let low = self.b.constant(target, target.truncate(low), at);
        Ok(self.b.binary(BinOp::Add, reduced, low, at))
    }
}
