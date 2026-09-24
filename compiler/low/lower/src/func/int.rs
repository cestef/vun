//! Integer arithmetic at run time (spec 04, rules 1 and 6): in the storage of the result's
//! range; a division in the storage holding its operands and its result, so that no quotient
//! overflows, its divisor checked where its range holds 0.

use vun_lir::{Access, BinOp, Cond, Scalar, Value};
use vun_mir::Operand;
use vun_sema::int::{Integer, Interval};
use vun_sema::ty::Ty;

use super::FnLower;
use crate::unlowered::{Form, Unlowered};

impl FnLower<'_, '_, '_> {
    /// `x op y` in the storage of the result's range `ty`.
    pub(crate) fn arithmetic(
        &mut self,
        op: BinOp,
        x: &Operand,
        y: &Operand,
        dest: Option<Value>,
        ty: Ty,
    ) -> Result<(), Unlowered> {
        let Some(dest) = dest else { return Ok(()) };
        let (int, signed) = self.l.types.int(ty).ok_or(Unlowered::unsupported(self.at, Form::Primitive))?;
        let (x, y) = (self.int_operand(x, int)?, self.int_operand(y, int)?);
        let op = if op == BinOp::LShr && signed { BinOp::AShr } else { op };
        let value = self.b.binary(op, x, y, self.at);
        self.b.store(value, dest, Access::Plain, self.at);
        Ok(())
    }

    /// The integer `operand` in the storage `int`.
    pub(crate) fn int_operand(&mut self, operand: &Operand, int: Scalar) -> Result<Value, Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Primitive);
        let ty = self.operand_ty(operand).ok_or_else(unsupported)?;
        let (own, signed) = self.l.types.int(ty).ok_or_else(unsupported)?;
        let value = self.scalar_operand(operand)?;
        Ok(self.resize(value, own, signed, int))
    }

    /// `x / y`, or `x % y` where not `quotient`, rounding toward zero.
    pub(crate) fn divide(
        &mut self,
        quotient: bool,
        x: &Operand,
        y: &Operand,
        dest: Option<Value>,
        ty: Ty,
    ) -> Result<(), Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Primitive);
        let divisor = self.operand_ty(y).ok_or_else(unsupported)?;
        let dividend = self.operand_ty(x).ok_or_else(unsupported)?;
        let (int, signed) = self.l.types.common_of(&[dividend, divisor, ty]).ok_or_else(unsupported)?;
        let (a, b) = (self.int_operand(x, int)?, self.int_operand(y, int)?);
        if divisor.interval().is_some_and(|r| r.contains(&Interval::singleton(Integer::zero()))) {
            let zero = self.b.constant(int, 0, self.at);
            let nonzero = self.b.compare(Cond::Ne, b, zero, self.at);
            self.check(nonzero)?;
        }
        let op = match (quotient, signed) {
            (true, true) => BinOp::SDiv,
            (true, false) => BinOp::UDiv,
            (false, true) => BinOp::SRem,
            (false, false) => BinOp::URem,
        };
        let value = self.b.binary(op, a, b, self.at);
        let Some(dest) = dest else { return Ok(()) };
        let (own, _) = self.l.types.int(ty).ok_or_else(unsupported)?;
        let value = self.resize(value, int, signed, own);
        self.b.store(value, dest, Access::Plain, self.at);
        Ok(())
    }
}
