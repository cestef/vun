//! Floats (spec 04, section 2): held as their bits in general registers, like integers; the
//! operations are snippets through `v0` and `v1`. arm64 converts between floats and unsigned
//! 64-bit integers directly.

use vun_asm_arm64::Size;
use vun_backend::{Lack, Missing};
use vun_lir::{BinOp, CastOp, Cond, Scalar, Value};

use super::{Sel, narrow};
use crate::arm64::condition;
use crate::arm64::snippets::{
    DOUBLE_TO_SIGNED, DOUBLE_TO_UNSIGNED, FADD, FDIV, FMUL, FNE, FSET, FSUB, NARROW, NARROW_SIGNED_TO_DOUBLE,
    NARROW_SIGNED_TO_SINGLE, NARROW_UNSIGNED_TO_DOUBLE, NARROW_UNSIGNED_TO_SINGLE, SIGNED_TO_DOUBLE,
    SIGNED_TO_SINGLE, SINGLE_TO_SIGNED, SINGLE_TO_UNSIGNED, UNSIGNED_TO_DOUBLE, UNSIGNED_TO_SINGLE, WIDEN,
};

impl Sel<'_, '_> {
    /// The size of the bits of the float scalar `scalar`, which is its precision.
    fn precision(&self, scalar: Scalar) -> Result<Size, Lack> {
        match scalar {
            Scalar::F32 => Ok(Size::B32),
            Scalar::F64 => Ok(Size::B64),
            _ => Err(self.lack(Missing::Half)),
        }
    }

    pub(super) fn float_binary(
        &mut self,
        op: BinOp,
        a: Value,
        b: Value,
        out: Option<Value>,
    ) -> Result<(), Lack> {
        let snippet = match op {
            BinOp::FAdd => FADD,
            BinOp::FSub => FSUB,
            BinOp::FMul => FMUL,
            _ => FDIV,
        };
        let p = self.precision(self.f.ty(a))?;
        self.apply(snippet, &[a, b], out, p, None);
        Ok(())
    }

    /// A float comparison, ordered as LIR's are: false where either operand is a NaN.
    pub(super) fn float_compare(
        &mut self,
        cond: Cond,
        a: Value,
        b: Value,
        out: Option<Value>,
    ) -> Result<(), Lack> {
        let p = self.precision(self.f.ty(a))?;
        match condition(cond) {
            Some(cc) => self.apply(FSET, &[a, b], out, p, Some(cc)),
            None => self.apply(FNE, &[a, b], out, p, None),
        }
        Ok(())
    }

    /// Conversions between floats, and between floats and integers; a narrower integer result
    /// is the low bits of the 64-bit one.
    pub(super) fn float_cast(
        &mut self,
        op: CastOp,
        value: Value,
        to: Scalar,
        out: Option<Value>,
    ) -> Result<(), Lack> {
        let from = self.f.ty(value);
        match op {
            CastOp::FConvert => {
                let snippet = if self.precision(to)? == Size::B64 { WIDEN } else { NARROW };
                self.precision(from)?;
                self.apply(snippet, &[value], out, Size::B64, None);
            }
            CastOp::SToF | CastOp::UToF => {
                let (p, size) = (self.precision(to)?, self.size(value)?);
                let single = p == Size::B32;
                let snippet = match (op == CastOp::SToF, narrow(size), single) {
                    (true, false, true) => SIGNED_TO_SINGLE,
                    (true, false, false) => SIGNED_TO_DOUBLE,
                    (false, false, true) => UNSIGNED_TO_SINGLE,
                    (false, false, false) => UNSIGNED_TO_DOUBLE,
                    (true, true, true) => NARROW_SIGNED_TO_SINGLE,
                    (true, true, false) => NARROW_SIGNED_TO_DOUBLE,
                    (false, true, true) => NARROW_UNSIGNED_TO_SINGLE,
                    (false, true, false) => NARROW_UNSIGNED_TO_DOUBLE,
                };
                self.apply(snippet, &[value], out, size, None);
            }
            _ => {
                let p = self.precision(from)?;
                self.size_of(to)?;
                let snippet = match (op == CastOp::FToS, p == Size::B32) {
                    (true, true) => SINGLE_TO_SIGNED,
                    (true, false) => DOUBLE_TO_SIGNED,
                    (false, true) => SINGLE_TO_UNSIGNED,
                    (false, false) => DOUBLE_TO_UNSIGNED,
                };
                self.apply(snippet, &[value], out, p, None);
            }
        }
        Ok(())
    }
}
