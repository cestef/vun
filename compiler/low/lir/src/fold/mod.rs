//! Folding operations on constants, as the builder makes them: integers only, and only on the
//! operands an operation is defined on (`Domain`), so a folded value is the one every backend
//! computes. Floats are left to the machine, whose NaNs a fold would have to match.

use crate::facts::{Domain, Reads};
use crate::inst::{BinOp, CastOp, Cond};
use crate::ty::Scalar;

mod function;
#[cfg(test)]
mod tests;

impl Scalar {
    /// The integer `bits` stand for in this integer type, read as signed.
    #[must_use]
    pub fn signed(self, bits: u128) -> i128 {
        let shift = 128 - self.bits(0);
        #[expect(clippy::cast_possible_wrap, reason = "two's complement bits")]
        let wide = (bits << shift) as i128;
        wide >> shift
    }
}

impl BinOp {
    /// `a op b` on constants of the integer type `ty`, where the operation is defined on them.
    #[must_use]
    pub fn fold(self, ty: Scalar, a: u128, b: u128) -> Option<u128> {
        if !ty.is_int() {
            return None;
        }
        let width = ty.bits(0);
        let defined = match self.domain() {
            Domain::Any => true,
            Domain::ShiftBelowWidth => b < u128::from(width),
            Domain::Divisor => b != 0,
            Domain::SignedDivisor => {
                b != 0 && !(ty.signed(b) == -1 && ty.signed(a) == ty.signed(1 << (width - 1)))
            }
            Domain::Fits => false,
        };
        if !defined {
            return None;
        }
        let (sa, sb) = (ty.signed(a), ty.signed(b));
        let bits = match self {
            Self::Add => a.wrapping_add(b),
            Self::Sub => a.wrapping_sub(b),
            Self::Mul => a.wrapping_mul(b),
            Self::And => a & b,
            Self::Or => a | b,
            Self::Xor => a ^ b,
            Self::Shl => a << b,
            Self::LShr => a >> b,
            Self::AShr => ty.truncate(sa >> b),
            Self::UDiv => a / b,
            Self::URem => a % b,
            Self::SDiv => ty.truncate(sa.wrapping_div(sb)),
            Self::SRem => ty.truncate(sa.wrapping_rem(sb)),
            Self::FAdd | Self::FSub | Self::FMul | Self::FDiv => return None,
        };
        Some(ty.truncate(bits.cast_signed()))
    }
}

impl Cond {
    /// Whether the comparison holds of constants of the integer type `ty`.
    #[must_use]
    pub fn fold(self, ty: Scalar, a: u128, b: u128) -> Option<bool> {
        if !ty.is_int() {
            return None;
        }
        let order = match self.reads() {
            Reads::Signed => ty.signed(a).cmp(&ty.signed(b)),
            Reads::Unsigned | Reads::Bits => a.cmp(&b),
            Reads::Float => return None,
        };
        Some(match self {
            Self::Eq => a == b,
            Self::Ne => a != b,
            Self::SLt | Self::ULt => order.is_lt(),
            Self::SLe | Self::ULe => order.is_le(),
            Self::SGt | Self::UGt => order.is_gt(),
            Self::SGe | Self::UGe => order.is_ge(),
            Self::FEq | Self::FNe | Self::FLt | Self::FLe | Self::FGt | Self::FGe => return None,
        })
    }
}

impl CastOp {
    /// The constant `bits` of `from` converted to `to`, for a conversion between integers or of
    /// the same bits.
    #[must_use]
    pub fn fold(self, from: Scalar, to: Scalar, bits: u128) -> Option<u128> {
        match self {
            Self::ZExt | Self::Trunc if from.is_int() && to.is_int() => Some(to.truncate(bits.cast_signed())),
            Self::SExt if from.is_int() && to.is_int() => Some(to.truncate(from.signed(bits))),
            Self::Bitcast if from != Scalar::Ptr && to != Scalar::Ptr => Some(bits),
            _ => None,
        }
    }
}
