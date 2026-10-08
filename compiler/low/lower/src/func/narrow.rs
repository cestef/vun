//! Integers brought into another integer type (spec 04, rule 9): checked, reduced or clamped;
//! and whether one fits a type, as `lang.int.fits` asks.

use vun_desugar::Narrowing;
use vun_lir::{Access, BinOp, Cond, Scalar, Value};
use vun_mir::Operand;
use vun_sema_int::{Extended, Integer, Interval};
use vun_sema_ty::Ty;
use vun_sema_ty::trap::Reason;

use super::FnLower;
use crate::repr::Types;
use crate::unlowered::{Form, Unlowered};

/// The bounds of a target range that cut a source range: those inside it, which fit the
/// source's storage.
pub(super) struct Cuts {
    pub(super) lo: Option<i128>,
    pub(super) hi: Option<i128>,
    pub(super) apart: Option<Apart>,
}

/// A target range with no value in common with the source's: its nearest bound, which
/// saturating gives, and which the source's storage need not hold.
#[derive(Clone, Copy)]
pub(super) struct Apart(pub(super) i128);

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
                    self.check(inside, &Reason::Narrowing)?;
                }
                self.resize(x, source, signed, target)
            }
            Narrowing::Sat => {
                if let Some(Apart(bound)) = cuts.apart {
                    self.b.constant(target, target.truncate(bound), at)
                } else {
                    let x = self.clamp(x, source, signed, &cuts);
                    self.resize(x, source, signed, target)
                }
            }
            // A value already of the range is its own reduction.
            Narrowing::Wrap if cuts.lo.is_none() && cuts.hi.is_none() && cuts.apart.is_none() => {
                self.resize(x, source, signed, target)
            }
            Narrowing::Wrap => {
                let own = self.operand_ty(operand).and_then(|t| t.interval()).ok_or_else(unsupported)?;
                let range = ty.interval().ok_or_else(unsupported)?;
                self.wrap(x, (source, signed, &own), &range, target)?
            }
        };
        self.keep(value, dest);
        Ok(())
    }

    /// `x`, held in `source` with values of `own`, reduced modulo the size of `range` into it,
    /// held in `target` (spec 04, rule 3).
    fn wrap(
        &mut self,
        x: Value,
        (source, signed, own): (Scalar, bool, &Interval),
        range: &Interval,
        target: Scalar,
    ) -> Result<Value, Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Conversion);
        let bound = |e: &Extended| e.finite().cloned().ok_or_else(unsupported);
        let (lo, hi) = (bound(range.lo())?, bound(range.hi())?);
        let span = &hi - &lo;
        let size = &span + &Integer::from(1);
        let bits = |s: Scalar| s.bits(self.l.types.pointer_bits());
        let exponent = u32::try_from(size.magnitude_bits() - 1).map_err(|_| unsupported())?;
        // A size that is a power of two, 2^128 included: in the wider of both storages, whose
        // wrapping keeps the low bits exact, the offset from the low bound masked.
        if size == Integer::power_of_two(exponent) {
            let (lo, span) = (lo.to_bits().ok_or_else(unsupported)?, span.to_bits().ok_or_else(unsupported)?);
            let wide = if bits(target) > bits(source) { target } else { source };
            let x = self.resize(x, source, signed, wide);
            let (low, mask) = (
                self.b.constant(wide, wide.truncate(lo), self.at),
                self.b.constant(wide, wide.truncate(span), self.at),
            );
            let shifted = self.b.binary(BinOp::Sub, x, low, self.at);
            let reduced = self.b.binary(BinOp::And, shifted, mask, self.at);
            let value = self.b.binary(BinOp::Add, reduced, low, self.at);
            return Ok(self.resize(value, wide, signed, target));
        }
        // Any other size: the offset from the low bound, exact in a storage holding its values
        // and the size, taken modulo the size, a negative remainder moved up by it.
        let offsets = own.sub(&Interval::singleton(lo.clone()));
        let reach = Interval::closed(Integer::zero(), size.clone()).ok_or_else(unsupported)?;
        let Some((wide, wide_signed)) = Types::storing(&offsets.hull(&reach)) else {
            return self.wrap_wide(x, (source, signed, own), (&lo, &size), target);
        };
        let (lo, size) = (lo.to_bits().ok_or_else(unsupported)?, size.to_bits().ok_or_else(unsupported)?);
        let x = self.resize(x, source, signed, wide);
        let (low, modulus) = (
            self.b.constant(wide, wide.truncate(lo), self.at),
            self.b.constant(wide, wide.truncate(size), self.at),
        );
        let offset = self.b.binary(BinOp::Sub, x, low, self.at);
        let rem = if wide_signed { BinOp::SRem } else { BinOp::URem };
        let mut reduced = self.b.binary(rem, offset, modulus, self.at);
        if wide_signed {
            let zero = self.b.constant(wide, 0, self.at);
            let below = self.b.compare(Cond::SLt, reduced, zero, self.at);
            let moved = self.b.binary(BinOp::Add, reduced, modulus, self.at);
            reduced = self.b.select(below, moved, reduced, self.at);
        }
        let reduced = self.resize(reduced, wide, false, target);
        let low = self.b.constant(target, target.truncate(lo), self.at);
        Ok(self.b.binary(BinOp::Add, reduced, low, self.at))
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
        let bound = |e: &Extended| e.finite().and_then(Integer::to_bits).ok_or_else(unsupported);
        // Only a bound inside the source's range cuts it, and such a bound fits the source's
        // storage: checks and clamps compare there, and nothing widens past both storages.
        let apart = if range.hi() < own.lo() {
            Some(Apart(bound(range.hi())?))
        } else if range.lo() > own.hi() {
            Some(Apart(bound(range.lo())?))
        } else {
            None
        };
        let (lo, hi) = if apart.is_some() {
            (None, None)
        } else {
            let lo = if range.lo() > own.lo() { Some(bound(range.lo())?) } else { None };
            let hi = if range.hi() < own.hi() { Some(bound(range.hi())?) } else { None };
            (lo, hi)
        };
        let x = self.b.load(source, temp, Access::Plain, self.at);
        Ok((x, source, signed, Cuts { lo, hi, apart }))
    }

    /// Whether the integer `operand` is a value of `ty`: 1 or 0, an `i8`; `None` when every value
    /// of its type is.
    pub(super) fn within(&mut self, operand: &Operand, ty: Ty) -> Result<Option<Value>, Unlowered> {
        let (x, source, signed, cuts) = self.cut(operand, ty)?;
        Ok(self.inside(x, source, signed, &cuts))
    }

    /// `x`, in the storage `source`, clamped to `cuts`.
    pub(super) fn clamp(&mut self, x: Value, source: Scalar, signed: bool, cuts: &Cuts) -> Value {
        let (lt, gt) = if signed { (Cond::SLt, Cond::SGt) } else { (Cond::ULt, Cond::UGt) };
        let mut x = x;
        for (cond, n) in [(lt, cuts.lo), (gt, cuts.hi)] {
            let Some(n) = n else { continue };
            let n = self.b.constant(source, source.truncate(n), self.at);
            let past = self.b.compare(cond, x, n, self.at);
            x = self.b.select(past, n, x, self.at);
        }
        x
    }

    /// Whether `x`, in the storage `source`, is within `cuts`: `None` when nothing cuts.
    fn inside(&mut self, x: Value, source: Scalar, signed: bool, cuts: &Cuts) -> Option<Value> {
        if cuts.apart.is_some() {
            return Some(self.b.constant(Scalar::I8, 0, self.at));
        }
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
