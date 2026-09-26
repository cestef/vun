//! Integers brought into another integer type (spec 04, rule 9): checked, reduced or clamped;
//! and whether one fits a type, as `lang.int.fits` asks.

use vun_desugar::Narrowing;
use vun_lir::{Access, BinOp, Cond, Scalar, Value};
use vun_mir::Operand;
use vun_sema::int::{Extended, Integer};
use vun_sema::ty::Ty;

use super::FnLower;
use crate::unlowered::{Form, Unlowered};

/// The bounds of a target range that cut a source range: those inside it, which fit the
/// source's storage.
struct Cuts {
    lo: Option<i128>,
    hi: Option<i128>,
}

impl FnLower<'_, '_, '_> {
    /// `operand` brought into the integer type `ty` (spec 04, rule 9): trapping outside it,
    /// reduced modulo its size, or clamped to its bounds.
    pub(crate) fn narrow(
        &mut self,
        operand: &Operand,
        narrowing: Narrowing,
        dest: Option<Value>,
        ty: Ty,
    ) -> Result<(), Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Conversion);
        let (target, _) = self.l.types.int(ty).ok_or_else(unsupported)?;
        let (x, source, signed, cuts) = self.cut(operand, ty)?;
        let value = match narrowing {
            Narrowing::To => {
                if let Some(inside) = self.inside(x, source, signed, &cuts) {
                    self.check(inside)?;
                }
                self.resize(x, source, signed, target)
            }
            Narrowing::Sat => {
                let (lt, gt) = if signed { (Cond::SLt, Cond::SGt) } else { (Cond::ULt, Cond::UGt) };
                let mut x = x;
                for (cond, n) in [(lt, cuts.lo), (gt, cuts.hi)] {
                    let Some(n) = n else { continue };
                    let n = self.b.constant(source, source.truncate(n), self.at);
                    let past = self.b.compare(cond, x, n, self.at);
                    x = self.b.select(past, n, x, self.at);
                }
                self.resize(x, source, signed, target)
            }
            // Modular reduction into a range whose size is a power of two, from its low bound,
            // in the wider of both storages, whose wrapping keeps the low bits exact.
            Narrowing::Wrap => {
                let range = ty.interval().ok_or_else(unsupported)?;
                let bound = |e: &Extended| e.finite().and_then(Integer::to_i128).ok_or_else(unsupported);
                let (lo, hi) = (bound(range.lo())?, bound(range.hi())?);
                // The size less one, a mask of low bits when the size is a power of two, 2^128
                // included.
                let span = hi.abs_diff(lo);
                if span & span.wrapping_add(1) != 0 {
                    return Err(unsupported());
                }
                let bits = |s: Scalar| s.bits(self.l.types.pointer_bits());
                let wide = if bits(target) > bits(source) { target } else { source };
                let x = self.resize(x, source, signed, wide);
                let (low, mask) = (
                    self.b.constant(wide, wide.truncate(lo), self.at),
                    self.b.constant(wide, wide.truncate(hi.wrapping_sub(lo)), self.at),
                );
                let shifted = self.b.binary(BinOp::Sub, x, low, self.at);
                let reduced = self.b.binary(BinOp::And, shifted, mask, self.at);
                let value = self.b.binary(BinOp::Add, reduced, low, self.at);
                self.resize(value, wide, signed, target)
            }
        };
        self.keep(value, dest);
        Ok(())
    }

    /// The integer `operand` loaded in its storage, with that storage and whether it is signed,
    /// and the bounds of `ty` that cut its range.
    fn cut(&mut self, operand: &Operand, ty: Ty) -> Result<(Value, Scalar, bool, Cuts), Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Conversion);
        let from = self.operand_ty(operand).ok_or_else(unsupported)?;
        let temp = self.temp(from)?.ok_or_else(unsupported)?;
        self.write(operand, Some(temp), from)?;
        let (source, signed) = self.l.types.int(from).ok_or_else(unsupported)?;
        let (range, own) = (ty.interval().ok_or_else(unsupported)?, from.interval().ok_or_else(unsupported)?);
        let bound = |e: &Extended| e.finite().and_then(Integer::to_i128).ok_or_else(unsupported);
        // Only a bound inside the source's range cuts it, and such a bound fits the source's
        // storage: checks and clamps compare there, and nothing widens past both storages.
        let lo = if range.lo() > own.lo() { Some(bound(range.lo())?) } else { None };
        let hi = if range.hi() < own.hi() { Some(bound(range.hi())?) } else { None };
        let x = self.b.load(source, temp, Access::Plain, self.at);
        Ok((x, source, signed, Cuts { lo, hi }))
    }

    /// Whether the integer `operand` is a value of `ty`: 1 or 0, an `i8`; `None` when every value
    /// of its type is.
    pub(super) fn within(&mut self, operand: &Operand, ty: Ty) -> Result<Option<Value>, Unlowered> {
        let (x, source, signed, cuts) = self.cut(operand, ty)?;
        Ok(self.inside(x, source, signed, &cuts))
    }

    /// Whether `x`, in the storage `source`, is within `cuts`: `None` when nothing cuts.
    fn inside(&mut self, x: Value, source: Scalar, signed: bool, cuts: &Cuts) -> Option<Value> {
        let (ge, le) = if signed { (Cond::SGe, Cond::SLe) } else { (Cond::UGe, Cond::ULe) };
        let mut inside = None;
        for (cond, n) in [(ge, cuts.lo), (le, cuts.hi)] {
            let Some(n) = n else { continue };
            let n = self.b.constant(source, source.truncate(n), self.at);
            let holds = self.b.compare(cond, x, n, self.at);
            inside = Some(match inside {
                Some(both) => self.b.binary(BinOp::And, both, holds, self.at),
                None => holds,
            });
        }
        inside
    }
}
