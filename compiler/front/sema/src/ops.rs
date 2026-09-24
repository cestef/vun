//! What operators compute (spec 04): on compile-time values, where integer arithmetic is exact and
//! a division by zero is a trap, and on integer types, by interval arithmetic. Sema and the
//! interpreter both compute through here.

use overt_grammar::TokenKind;

use crate::env::Unevaluated;
use crate::int::Interval;
use crate::ty::Value;

impl Value {
    /// `left op right`.
    ///
    /// # Errors
    /// Returns [`Unevaluated::Trapped`] for a division by zero or a shift past any width, and
    /// [`Unevaluated::Unsupported`] for operands or an operator not computed yet.
    pub fn binary(op: TokenKind, left: &Self, right: &Self) -> Result<Self, Unevaluated> {
        let (Self::Int(a), Self::Int(b)) = (left, right) else { return Err(Unevaluated::Unsupported) };
        let n = match op {
            TokenKind::Plus => a + b,
            TokenKind::Minus => a - b,
            TokenKind::Star => a * b,
            TokenKind::Slash => a.checked_div(b).ok_or(Unevaluated::Trapped)?,
            TokenKind::Percent => a.checked_rem(b).ok_or(Unevaluated::Trapped)?,
            TokenKind::Shl => a.shifted(b).ok_or(Unevaluated::Trapped)?,
            TokenKind::Shr => a.shifted(&-b).ok_or(Unevaluated::Trapped)?,
            _ => return Err(Unevaluated::Unsupported),
        };
        Ok(Self::Int(n))
    }

    /// Whether the comparison `left op right` of two integers holds; `None` for another operator
    /// or other operands.
    #[must_use]
    pub fn compare(op: TokenKind, left: &Self, right: &Self) -> Option<bool> {
        let (Self::Int(a), Self::Int(b)) = (left, right) else { return None };
        Some(match op {
            TokenKind::EqEq => a == b,
            TokenKind::BangEq => a != b,
            TokenKind::Lt => a < b,
            TokenKind::Le => a <= b,
            TokenKind::Gt => a > b,
            TokenKind::Ge => a >= b,
            _ => return None,
        })
    }

    /// `op operand`.
    ///
    /// # Errors
    /// Returns [`Unevaluated::Unsupported`] for an operand or an operator not computed yet.
    pub fn unary(op: TokenKind, operand: &Self) -> Result<Self, Unevaluated> {
        match (op, operand) {
            (TokenKind::Minus, Self::Int(n)) => Ok(Self::Int(-n)),
            _ => Err(Unevaluated::Unsupported),
        }
    }

    /// `lo..hi`, `lo..=hi` or `lo..`, by `op`.
    ///
    /// # Errors
    /// Returns [`Unevaluated::Unsupported`] for bounds that are not integers.
    pub fn range(op: TokenKind, lo: &Self, hi: Option<&Self>) -> Result<Self, Unevaluated> {
        let Self::Int(lo) = lo else { return Err(Unevaluated::Unsupported) };
        let range = match (hi, op) {
            (None, _) => Some(Interval::at_least(lo.clone())),
            (Some(Self::Int(hi)), TokenKind::DotDotEq) => Interval::closed(lo.clone(), hi.clone()),
            (Some(Self::Int(hi)), _) => Interval::half_open(lo.clone(), hi),
            (Some(_), _) => return Err(Unevaluated::Unsupported),
        };
        Ok(Self::Range(range))
    }
}

impl Interval {
    /// The range of `self op other` over integers of these ranges (rule 1); `None` for an
    /// operator not typed yet, or a division by a divisor that is always zero.
    #[must_use]
    pub fn binary(&self, op: TokenKind, other: &Self) -> Option<Self> {
        match op {
            TokenKind::Plus => Some(self.add(other)),
            TokenKind::Minus => Some(self.sub(other)),
            TokenKind::Star => Some(self.mul(other)),
            TokenKind::Slash => self.div(other).range,
            TokenKind::Percent => self.rem(other).range,
            _ => None,
        }
    }
}
