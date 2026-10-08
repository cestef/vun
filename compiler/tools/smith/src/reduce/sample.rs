//! A value of each type to stand for an expression of it: the least in magnitude.

use crate::int::{BigInt, Range};
use crate::ir::{Expr, Kind};
use crate::ty::Ty;
use crate::value::Value;

impl Range {
    /// The value of the range nearest zero.
    #[must_use]
    pub fn nearest_zero(&self) -> BigInt {
        if self.contains(&BigInt::ZERO) {
            BigInt::ZERO
        } else if self.lo > BigInt::ZERO {
            self.lo.clone()
        } else {
            self.hi.clone()
        }
    }
}

impl Expr {
    /// An expression of type `ty` with the least value, in the form of a literal.
    #[must_use]
    pub fn sample(ty: &Ty) -> Self {
        let kind = match ty {
            Ty::Int(range) => Kind::Lit(Value::Int(range.nearest_zero())),
            Ty::Bool => Kind::Lit(Value::Bool(false)),
            Ty::Ref(..) | Ty::Closure(..) | Ty::Param(_) | Ty::Unit => Kind::Lit(Value::Unit),
            Ty::Float(_) => Kind::Lit(Value::Float(crate::float::Float::of(0.0))),
            Ty::Tuple(parts) => Kind::Tuple(parts.iter().map(Self::sample).collect()),
            Ty::Array(count, item) => Kind::Repeat(Box::new(Self::sample(item)), *count),
            Ty::Struct(id, fields) => Kind::Struct(*id, fields.iter().map(Self::sample).collect()),
            Ty::Enum(nominal, variants) => {
                Kind::Variant(*nominal, 0, variants[0].iter().map(Self::sample).collect())
            }
        };
        Self { ty: ty.clone(), kind }
    }

    /// Whether it is a literal with a sign in front, of the one value its type holds, which the
    /// sample writes as the same text.
    fn negated_literal(&self) -> bool {
        matches!(&self.kind, Kind::Unary(_, inner) if matches!(inner.kind, Kind::Lit(_)))
            && self.ty.range().is_some_and(|range| range.count() == BigInt::from(1))
    }

    /// Whether it is already the sample of its type, which has nothing smaller to be replaced by.
    #[must_use]
    pub fn is_sample(&self) -> bool {
        matches!(self.kind, Kind::Lit(_))
            || matches!(self.ty, Ty::Ref(..) | Ty::Closure(..))
            || self.ty.has_param()
            || self.negated_literal()
            || self.is_shaped_like(&Self::sample(&self.ty))
    }

    /// Whether it is `sample` but for the types of the literals in it, which a payload narrower than
    /// the type it is in has.
    fn is_shaped_like(&self, sample: &Self) -> bool {
        let parts = |ours: &[Self], theirs: &[Self]| {
            ours.len() == theirs.len()
                && ours.iter().zip(theirs).all(|(ours, theirs)| ours.is_shaped_like(theirs))
        };
        match (&self.kind, &sample.kind) {
            (Kind::Lit(ours), Kind::Lit(theirs)) => ours == theirs,
            (Kind::Tuple(ours), Kind::Tuple(theirs)) => parts(ours, theirs),
            (Kind::Struct(id, ours), Kind::Struct(other, theirs)) => id == other && parts(ours, theirs),
            (Kind::Variant(nominal, at, ours), Kind::Variant(other, there, theirs)) => {
                nominal == other && at == there && parts(ours, theirs)
            }
            (Kind::Repeat(ours, n), Kind::Repeat(theirs, m)) => n == m && ours.is_shaped_like(theirs),
            _ => self == sample,
        }
    }
}
