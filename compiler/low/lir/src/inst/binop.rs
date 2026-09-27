//! Binary operations, and what they give where one operand is known.

use crate::ty::Scalar;
use vun_codec::{Decode, Encode};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    /// Division rounding toward zero, of signed or unsigned operands.
    SDiv,
    UDiv,
    /// The remainder of the division of the same name.
    SRem,
    URem,
    And,
    Or,
    Xor,
    /// Shifts by an amount below the width.
    Shl,
    LShr,
    AShr,
    FAdd,
    FSub,
    FMul,
    FDiv,
}

impl BinOp {
    /// The right operand leaving the left unchanged, for an integer operation of type `ty` that
    /// has one. Floats have none: `-0.0 + 0.0` is `0.0`.
    #[must_use]
    pub fn identity(self, ty: Scalar) -> Option<u128> {
        match self {
            Self::Add | Self::Sub | Self::Or | Self::Xor | Self::Shl | Self::LShr | Self::AShr => Some(0),
            Self::Mul | Self::SDiv | Self::UDiv => Some(1),
            Self::And => ty.is_int().then(|| ty.truncate(-1)),
            Self::SRem | Self::URem | Self::FAdd | Self::FSub | Self::FMul | Self::FDiv => None,
        }
    }

    /// Whether the operands may be swapped: an integer sum, product or bitwise operation.
    #[must_use]
    pub const fn commutes(self) -> bool {
        matches!(self, Self::Add | Self::Mul | Self::And | Self::Or | Self::Xor)
    }

    /// The right operand making the result itself whatever the left is, as 0 does of `x * 0`.
    #[must_use]
    pub fn absorbing(self, ty: Scalar) -> Option<u128> {
        match self {
            Self::Mul | Self::And => ty.is_int().then_some(0),
            Self::Or => ty.is_int().then(|| ty.truncate(-1)),
            _ => None,
        }
    }

    /// What `a op b` is where only `b`, of type `ty`, is known.
    #[must_use]
    pub fn with_right(self, ty: Scalar, b: u128) -> Option<Right> {
        if self.identity(ty) == Some(b) {
            Some(Right::Left)
        } else {
            (self.absorbing(ty) == Some(b)).then_some(Right::Constant(b))
        }
    }
}

/// What an operation gives where its right operand alone is known.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Right {
    /// Its left operand, as `x * 1` does.
    Left,
    /// This constant, as `x * 0` does.
    Constant(u128),
}
