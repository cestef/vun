//! Integer arithmetic at run time (spec 04, rules 1 and 6): in the storage of the result's
//! range, one operand's too (narrowing, `~`, negation, `abs`, whether it fits a type); a division in the storage holding its operands and its result, so that no quotient
//! overflows, its divisor checked where its range holds 0.

use vun_desugar::Narrowing;
use vun_lir::{Access, BinOp, Cond, Scalar, Value};
use vun_mir::Operand;
use vun_primitives::int::Int;
use vun_sema_int::{Integer, Interval};
use vun_sema_ty::Ty;
use vun_sema_ty::trap::Reason;

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
        let (int, _) = self.l.types.int(ty).ok_or(Unlowered::unsupported(self.at, Form::Primitive))?;
        let (x, y) = (self.int_operand(x, int)?, self.int_operand(y, int)?);
        let value = self.b.binary(op, x, y, self.at);
        self.b.store(value, dest, Access::Plain, self.at);
        Ok(())
    }

    /// `x << n` or `x >> n`, of the result's range `ty`. Where that is a `uN` range, an amount past
    /// `N - 1` traps (rule 8), checked in the amount's own storage where its range does not
    /// prove it.
    pub(crate) fn shift(
        &mut self,
        op: BinOp,
        x: &Operand,
        n: &Operand,
        dest: Option<Value>,
        ty: Ty,
    ) -> Result<(), Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Primitive);
        // A shift of a compile-time value has the exact range typing found, which no amount leaves.
        let Some(width) = ty.interval().and_then(|r| r.unsigned_width()) else {
            return self.arithmetic(op, x, n, dest, ty);
        };
        let amount_ty = self.operand_ty(n).ok_or_else(unsupported)?;
        let (own, signed) = self.l.types.int(amount_ty).ok_or_else(unsupported)?;
        let amount = self.scalar_operand(n)?;
        let below = Interval::half_open(Integer::zero(), &Integer::from(i128::from(width)))
            .ok_or_else(unsupported)?;
        let proven = amount_ty.interval().is_some_and(|r| below.contains(&r));
        // An amount whose storage cannot hold `N` is below it, negative ones aside, which the
        // unsigned compare sees as past it.
        let fits = own.bits(self.l.types.pointer_bits()) >= u32::BITS - width.leading_zeros();
        if !proven && fits {
            let limit = self.b.constant(own, u128::from(width), at);
            let within = self.b.compare(Cond::ULt, amount, limit, at);
            self.check(within, &Reason::Shift)?;
        } else if !proven && signed {
            let zero = self.b.constant(own, 0, at);
            let within = self.b.compare(Cond::SGe, amount, zero, at);
            self.check(within, &Reason::Shift)?;
        }
        let Some(dest) = dest else { return Ok(()) };
        let (int, _) = self.l.types.int(ty).ok_or_else(unsupported)?;
        let x = self.int_operand(x, int)?;
        let amount = self.resize(amount, own, signed, int);
        let mut value = self.b.binary(op, x, amount, at);
        // A `uN` narrower than its storage keeps its low `N` bits: what a left shift moves past
        // them is dropped.
        if op == BinOp::Shl && width < int.bits(self.l.types.pointer_bits()) {
            let mask = self.b.constant(int, (1u128 << width) - 1, at);
            value = self.b.binary(BinOp::And, value, mask, at);
        }
        self.b.store(value, dest, Access::Plain, at);
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
            self.check(nonzero, &Reason::DivisionByZero)?;
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

    /// The primitives of `lang.int` of one run-time operand, `runtime` the run-time arguments of
    /// `args`, into `dest` of type `ty`.
    pub(super) fn int_unary(
        &mut self,
        i: Int,
        args: &[Operand],
        runtime: &[&Operand],
        dest: Option<Value>,
        ty: Ty,
    ) -> Result<(), Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Primitive);
        let x = runtime.first().copied().ok_or_else(unsupported)?;
        let narrowing = match i {
            Int::to => Some(Narrowing::To),
            Int::wrap => Some(Narrowing::Wrap),
            Int::sat => Some(Narrowing::Sat),
            _ => None,
        };
        if let Some(narrowing) = narrowing {
            return self.narrow(x, narrowing, dest, ty);
        }
        if i == Int::fits {
            let [_, _, Operand::Const(target)] = args else { return Err(unsupported()) };
            let target = target.as_type().ok_or_else(unsupported)?;
            let holds = match self.within(x, target)? {
                Some(holds) => holds,
                None => self.b.constant(Scalar::I8, 1, self.at),
            };
            return self.condition(holds, dest, ty);
        }
        let (int, _) = self.l.types.int(ty).ok_or_else(unsupported)?;
        let value = self.int_operand(x, int)?;
        let value = match i {
            Int::bit_not => {
                let hi = ty.interval().and_then(|r| r.hi().finite().and_then(Integer::to_u128));
                let mask = self.b.constant(int, hi.ok_or_else(unsupported)?, self.at);
                self.b.binary(BinOp::Xor, value, mask, self.at)
            }
            Int::neg => {
                let zero = self.b.constant(int, 0, self.at);
                self.b.binary(BinOp::Sub, zero, value, self.at)
            }
            // The magnitude, whose range holds the negation of the lowest value: the operand's
            // sign is read in its own storage.
            Int::abs => {
                let own = self.operand_ty(x).ok_or_else(unsupported)?;
                let (storage, signed) = self.l.types.int(own).ok_or_else(unsupported)?;
                if signed {
                    let raw = self.scalar_operand(x)?;
                    let zero = self.b.constant(storage, 0, self.at);
                    let negative = self.b.compare(Cond::SLt, raw, zero, self.at);
                    let zero = self.b.constant(int, 0, self.at);
                    let negated = self.b.binary(BinOp::Sub, zero, value, self.at);
                    self.b.select(negative, negated, value, self.at)
                } else {
                    value
                }
            }
            _ => return Err(unsupported()),
        };
        self.keep(value, dest);
        Ok(())
    }
}
