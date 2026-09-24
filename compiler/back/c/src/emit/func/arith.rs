//! Operations on scalars, as each operation's facts say it reads its operands (`Reads`): bits and
//! unsigned operands as `uN`, converted to `unsigned` first when narrower than `int`; signed ones
//! as `iN`; floats as they are. Unsigned arithmetic wraps as LIR's does, so no operation relies on
//! behavior C leaves undefined; LIR's domains (a shift below the width, a divisor that is not 0)
//! are guaranteed by lowering.

use vun_lir::{BinOp, CastOp, Cond, Reads, Scalar, Value};

use super::FnEmit;
use crate::ast::{Binary, Expr, Own, Ty};
use crate::lack::Lack;

impl FnEmit<'_, '_> {
    /// An operand read as `reads` reads a value of `s`.
    fn read(&mut self, v: Value, s: Scalar, reads: Reads) -> Expr {
        let e = self.operand(v);
        match reads {
            Reads::Signed => e.cast(Ty::int(s.bits(0))),
            Reads::Bits | Reads::Unsigned if s.is_int() && s.bits(0) < self.e.int_bits() => {
                e.cast(Ty::unsigned())
            }
            Reads::Bits | Reads::Unsigned | Reads::Float => e,
        }
    }

    pub(super) fn binary(&mut self, op: BinOp, lhs: Value, rhs: Value) -> Result<Expr, Lack> {
        let s = self.func.ty(lhs);
        let operator = match op {
            BinOp::Add | BinOp::FAdd => Binary::Add,
            BinOp::Sub | BinOp::FSub => Binary::Sub,
            BinOp::Mul | BinOp::FMul => Binary::Mul,
            BinOp::SDiv | BinOp::UDiv | BinOp::FDiv => Binary::Div,
            BinOp::SRem | BinOp::URem => Binary::Rem,
            BinOp::And => Binary::And,
            BinOp::Or => Binary::Or,
            BinOp::Xor => Binary::Xor,
            BinOp::Shl => Binary::Shl,
            BinOp::LShr | BinOp::AShr => Binary::Shr,
        };
        let (x, y) = (self.read(lhs, s, op.reads()), self.read(rhs, s, op.reads()));
        let e = Expr::binary(operator, x, y);
        Ok(if s.is_float() { e } else { e.cast(self.e.scalar(s, self.func.origin)?) })
    }

    pub(super) fn compare(&mut self, cond: Cond, lhs: Value, rhs: Value) -> Expr {
        let s = self.func.ty(lhs);
        // Addresses are compared as the integers they are.
        let reads = if s == Scalar::Ptr { Reads::Bits } else { cond.reads() };
        let (mut x, mut y) = (self.read(lhs, s, reads), self.read(rhs, s, reads));
        if s == Scalar::Ptr {
            (x, y) = (x.cast(self.e.word()), y.cast(self.e.word()));
        }
        let e = match cond {
            Cond::Eq | Cond::FEq => Expr::binary(Binary::Eq, x, y),
            // Ordered: false where either is not a number, as `!=` is not.
            Cond::FNe => Expr::binary(
                Binary::Either,
                Expr::binary(Binary::Lt, x.clone(), y.clone()),
                Expr::binary(Binary::Gt, x, y),
            ),
            Cond::Ne => Expr::binary(Binary::Ne, x, y),
            Cond::SLt | Cond::ULt | Cond::FLt => Expr::binary(Binary::Lt, x, y),
            Cond::SLe | Cond::ULe | Cond::FLe => Expr::binary(Binary::Le, x, y),
            Cond::SGt | Cond::UGt | Cond::FGt => Expr::binary(Binary::Gt, x, y),
            Cond::SGe | Cond::UGe | Cond::FGe => Expr::binary(Binary::Ge, x, y),
        };
        e.cast(Ty::BYTE)
    }

    pub(super) fn cast(&mut self, op: CastOp, v: Value, to: Scalar) -> Result<Expr, Lack> {
        let (from, at) = (self.func.ty(v), self.func.origin);
        let target = self.e.scalar(to, at)?;
        let e = self.operand(v);
        Ok(match op {
            CastOp::ZExt | CastOp::Trunc | CastOp::FConvert | CastOp::UToF | CastOp::FToU => e.cast(target),
            CastOp::SExt => e.cast(Ty::int(from.bits(0))).cast(Ty::int(to.bits(0))).cast(target),
            CastOp::SToF => e.cast(Ty::int(from.bits(0))).cast(target),
            CastOp::FToS => e.cast(Ty::int(to.bits(0))).cast(target),
            CastOp::PtrToInt | CastOp::IntToPtr => e.cast(self.e.word()).cast(target),
            CastOp::Bitcast => match (from.is_float(), to.is_float()) {
                (false, true) => self.helper(Own::FloatOf(to.bits(0)), e),
                (true, false) => self.helper(Own::BitsOf(from.bits(0)), e),
                _ if from == Scalar::Ptr || to == Scalar::Ptr => e.cast(self.e.word()).cast(target),
                _ => e,
            },
        })
    }

    /// A call of the float conversion `helper`, declared once it is used.
    fn helper(&mut self, helper: Own, arg: Expr) -> Expr {
        self.e.helpers.insert(helper);
        Expr::Call(Box::new(Expr::Name(crate::ast::Ident::Own(helper))), vec![arg])
    }
}
