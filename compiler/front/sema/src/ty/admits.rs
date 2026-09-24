//! Whether a compile-time value is of a type: every argument of a compile-time call must be a
//! value of its parameter's type, as a run-time one is (spec 03, section 7). A check reports only
//! what it knows: a value is outside a type when its structure shows it, and otherwise taken in.

use super::{Signature, Ty, TyKind};
use crate::int::Interval;
use crate::ty::Value;

impl Value {
    /// Whether this value is known not to be a value of `ty`: an integer outside its range, an
    /// array of another length, a sequence or tuple holding such a value, a value of another
    /// `struct` or `enum` type it does not accept. A shared borrow of a compile-time value reads the value, which
    /// is checked against the type borrowed.
    #[must_use]
    pub fn outside(&self, ty: Ty) -> bool {
        if let Some(pointee) = ty.borrowed() {
            return self.outside(pointee);
        }
        match (self, &*ty) {
            (Self::Int(n), _) => {
                ty.interval().is_some_and(|range| !range.contains(&Interval::singleton(n.clone())))
            }
            (Self::Array(items), _) => {
                if let Some((len, _)) = ty.as_array()
                    && len.to_i128().and_then(|n| usize::try_from(n).ok()) != Some(items.len())
                {
                    return true;
                }
                ty.items().is_some_and(|item| items.iter().any(|v| v.outside(item)))
            }
            (Self::Tuple(values), TyKind::Tuple(items)) => {
                values.len() != items.len() || values.iter().zip(items).any(|(v, &t)| v.outside(t))
            }
            (Self::Struct { ty: of, .. } | Self::Variant { ty: of, .. }, TyKind::Nominal(_)) => {
                matches!(**of, TyKind::Nominal(_)) && !ty.accepts(of)
            }
            _ => false,
        }
    }
}

impl Signature {
    /// Whether an argument of `args` is known not to be a value of its parameter's type.
    #[must_use]
    pub fn refuses(&self, args: &[Value]) -> bool {
        args.iter().zip(&self.params).any(|(arg, &param)| arg.outside(param))
    }
}
