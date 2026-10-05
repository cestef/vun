//! Conversions that round (spec 04, section 2): an integer to the float nearest it, a float to
//! one of another width, a NaN the canonical one, and a float toward zero to an integer, clamped
//! to its type and a NaN giving 0. Hardware and C leave a float outside the integer's range
//! undefined, so the float is brought inside before it converts and the bound put in after.

use vun_lir::{CastOp, Cond, Value};
use vun_mir::Operand;
use vun_primitives::float::Float;
use vun_sema_float::{Binary, Format};
use vun_sema_int::{Extended, Integer};
use vun_sema_ty::Ty;
use vun_sema_ty::float::FloatType;

use super::FnLower;
use super::narrow::Cuts;
use crate::unlowered::{Form, Unlowered};

impl FnLower<'_, '_, '_> {
    /// The conversion `f` of `operand` to the type `ty`, into `dest`.
    pub(super) fn rounded(
        &mut self,
        f: Float,
        operand: &Operand,
        dest: Option<Value>,
        ty: Ty,
    ) -> Result<(), Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Conversion);
        let from = self.operand_ty(operand).ok_or_else(unsupported)?;
        let x = self.scalar_operand(operand)?;
        let value = match f {
            Float::from_int => {
                let (_, signed) = self.l.types.int(from).ok_or_else(unsupported)?;
                let op = if signed { CastOp::SToF } else { CastOp::UToF };
                self.b.cast(op, x, self.scalar_of(ty)?, at)
            }
            Float::convert => {
                // A conversion to the same format changes no bit but a NaN's.
                let to = self.scalar_of(ty)?;
                let value = if self.b.ty(x) == to { x } else { self.b.cast(CastOp::FConvert, x, to, at) };
                self.canonical(value, Format::of(&ty).ok_or_else(unsupported)?)
            }
            Float::to_int => self.truncated(x, Format::of(&from).ok_or_else(unsupported)?, ty)?,
            _ => return Err(unsupported()),
        };
        self.keep(value, dest);
        Ok(())
    }

    /// The float `x`, of `format`, rounded toward zero into the integer type `ty`: converted
    /// within its storage's range, 0 in place of a NaN or a value past the top, the least value
    /// in place of one below, then the greatest put in above and the value clamped to `ty`.
    fn truncated(&mut self, x: Value, format: Format, ty: Ty) -> Result<Value, Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Conversion);
        let (int, signed) = self.l.types.int(ty).ok_or_else(unsupported)?;
        let bits = int.bits(self.l.types.pointer_bits());
        let float = self.b.ty(x);
        let constant = |this: &mut Self, n: &Integer| {
            let bits = Binary::of_integer(format, n).bits;
            this.b.constant(float, u128::from(bits), at)
        };
        let zero = constant(self, &Integer::zero());
        // The storage holds `least..past`; an unsigned one takes what truncates to 0 from above
        // -1.
        let (least, past) = if signed {
            let half = Integer::power_of_two(bits - 1);
            (-&half, half)
        } else {
            (Integer::from(-1), Integer::power_of_two(bits))
        };
        let (under, over) = if signed { (Cond::FLt, Cond::FGe) } else { (Cond::FLe, Cond::FGe) };
        let bound = constant(self, &least);
        let under = self.b.compare(under, x, bound, at);
        let top = constant(self, &past);
        let over = self.b.compare(over, x, top, at);
        let ordered = self.b.compare(Cond::FEq, x, x, at);
        let floor = if signed { bound } else { zero };
        let inside = self.b.select(under, floor, x, at);
        let inside = self.b.select(over, zero, inside, at);
        let inside = self.b.select(ordered, inside, zero, at);
        let op = if signed { CastOp::FToS } else { CastOp::FToU };
        let value = self.b.cast(op, inside, int, at);
        let greatest = &past - &Integer::from(1);
        let greatest = self.b.constant(int, int.truncate(greatest.to_i128().ok_or_else(unsupported)?), at);
        let value = self.b.select(over, greatest, value, at);
        // The bounds of `ty` that cut its storage's range.
        let range = ty.interval().ok_or_else(unsupported)?;
        let cut = |e: &Extended, edge: &Integer| e.finite().filter(|n| *n != edge).and_then(Integer::to_i128);
        let least = if signed { least } else { Integer::zero() };
        let cuts = Cuts { lo: cut(range.lo(), &least), hi: cut(range.hi(), &(&past - &Integer::from(1))) };
        Ok(self.clamp(value, int, signed, &cuts))
    }
}
