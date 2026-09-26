//! The primitives of `lang.float` at run time (spec 04, section 2): arithmetic as LIR's float
//! operations, a NaN result replaced by the canonical NaN; comparisons (`!=` as `==` negated,
//! since LIR's comparisons are ordered); negation as the sign bit flipped; `totalOrder` as a
//! signed comparison of the bits with the order of negative values reversed; and the encoding
//! through `bits` and `from_bits`. `rem` and `mul_add` run as their `lower.by` functions.

use vun_lir::{Access, BinOp, CastOp, Cond, Scalar, Value};
use vun_mir::Operand;
use vun_sema::float::{Binary, Format};
use vun_sema::primitives::float::Float;
use vun_sema::ty::Ty;

use super::FnLower;
use crate::unlowered::{Form, Unlowered};

impl FnLower<'_, '_, '_> {
    /// `f` applied to `runtime`, its run-time arguments, into `dest`, of type `ty`.
    pub(crate) fn float(
        &mut self,
        f: Float,
        runtime: &[&Operand],
        dest: Option<Value>,
        ty: Ty,
    ) -> Result<(), Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Primitive);
        let arg = |i: usize| runtime.get(i).copied().ok_or_else(unsupported);
        // LIR's float comparisons are ordered, false with a NaN; IEEE `!=` is the one true
        // there, the negation of `==`.
        let cond = match f {
            Float::equal => Some((Cond::FEq, false)),
            Float::unequal => Some((Cond::FEq, true)),
            Float::less => Some((Cond::FLt, false)),
            Float::less_equal => Some((Cond::FLe, false)),
            Float::greater => Some((Cond::FGt, false)),
            Float::greater_equal => Some((Cond::FGe, false)),
            _ => None,
        };
        if let Some((cond, negated)) = cond {
            let (x, y) = (self.float_through(arg(0)?)?, self.float_through(arg(1)?)?);
            let mut holds = self.b.compare(cond, x, y, self.at);
            if negated {
                let one = self.b.constant(self.b.ty(holds), 1, self.at);
                holds = self.b.binary(BinOp::Xor, holds, one, self.at);
            }
            return self.condition(holds, dest, ty);
        }
        if f == Float::total_order {
            let (x, y) = (self.float_through(arg(0)?)?, self.float_through(arg(1)?)?);
            let (x, y) = (self.total_key(x), self.total_key(y));
            let less = self.b.compare(Cond::SLt, x, y, self.at);
            let equal = self.b.compare(Cond::Eq, x, y, self.at);
            return self.ordering(less, equal, dest, ty);
        }
        // The encoding: the same bits as the other scalar.
        if let Float::bits | Float::from_bits = f {
            let x = self.scalar_operand(arg(0)?)?;
            let to = self.scalar_of(ty)?;
            let value = self.b.cast(CastOp::Bitcast, x, to, self.at);
            self.keep(value, dest);
            return Ok(());
        }
        let Some(dest) = dest else { return Ok(()) };
        let op = match f {
            Float::add => BinOp::FAdd,
            Float::sub => BinOp::FSub,
            Float::mul => BinOp::FMul,
            Float::div => BinOp::FDiv,
            Float::neg => {
                let x = self.scalar_operand(arg(0)?)?;
                let (bits, int) = self.bits(x);
                let width = int.bytes(0) * 8;
                let sign = self.b.constant(int, 1 << (width - 1), self.at);
                let flipped = self.b.binary(BinOp::Xor, bits, sign, self.at);
                let value = self.b.cast(CastOp::Bitcast, flipped, self.b.ty(x), self.at);
                self.b.store(value, dest, Access::Plain, self.at);
                return Ok(());
            }
            _ => return Err(unsupported()),
        };
        let (x, y) = (self.scalar_operand(arg(0)?)?, self.scalar_operand(arg(1)?)?);
        let value = self.b.binary(op, x, y, self.at);
        let format = Format::of(&ty).ok_or_else(unsupported)?;
        let value = self.canonical(value, format);
        self.b.store(value, dest, Access::Plain, self.at);
        Ok(())
    }

    /// `x`, or the canonical NaN of `format` where `x` is a NaN, whatever NaN the hardware gave
    /// (spec 04, section 2): only a NaN is unequal to itself.
    fn canonical(&mut self, x: Value, format: Format) -> Value {
        let ordered = self.b.compare(Cond::FEq, x, x, self.at);
        let nan = self.b.constant(self.b.ty(x), u128::from(Binary::nan(format).bits), self.at);
        self.b.select(ordered, x, nan, self.at)
    }

    /// The float a borrow `operand` reaches.
    fn float_through(&mut self, operand: &Operand) -> Result<Value, Unlowered> {
        let at = self.at;
        let ty = self
            .operand_ty(operand)
            .and_then(|t| t.pointee())
            .ok_or(Unlowered::unsupported(at, Form::Primitive))?;
        let addr = self.pointed(operand)?;
        self.load_scalar(addr, ty)
    }

    /// The bits of the float `x`, as the integer of its width.
    fn bits(&mut self, x: Value) -> (Value, Scalar) {
        let int =
            Scalar::int(u32::try_from(self.b.ty(x).bytes(0) * 8).expect("a float's width")).expect("a width");
        (self.b.cast(CastOp::Bitcast, x, int, self.at), int)
    }

    /// The bits of `x` with those of a negative value but the sign flipped: signed integers in
    /// the order of IEEE `totalOrder`.
    fn total_key(&mut self, x: Value) -> Value {
        let (bits, int) = self.bits(x);
        let top = self.b.constant(int, u128::from(int.bytes(0) * 8 - 1), self.at);
        let one = self.b.constant(int, 1, self.at);
        let sign = self.b.binary(BinOp::AShr, bits, top, self.at);
        let low = self.b.binary(BinOp::LShr, sign, one, self.at);
        self.b.binary(BinOp::Xor, bits, low, self.at)
    }
}
