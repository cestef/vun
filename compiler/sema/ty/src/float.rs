//! Float formats as types: `lang.Float(bits)` for each binary interchange format.

use vun_primitives::Primitive;
use vun_primitives::lang::Lang;
use vun_sema_float::Format;
use vun_sema_int::Integer;

use crate::{Ty, TyKind, Value};

/// A float format read from and written as its type.
pub trait FloatType: Sized {
    /// The format of the float type `ty`, if it is one.
    #[must_use]
    fn of(ty: &TyKind) -> Option<Self>;

    /// The float type of this format, `lang.Float(bits)`.
    #[must_use]
    fn ty(self) -> Ty;
}

impl FloatType for Format {
    fn of(ty: &TyKind) -> Option<Self> {
        match ty {
            TyKind::Primitive(Primitive::Lang(Lang::Float), args) => match args.as_slice() {
                [Value::Int(bits)] => Self::of_bits(bits),
                _ => None,
            },
            _ => None,
        }
    }

    fn ty(self) -> Ty {
        let bits = Value::Int(Integer::from(i128::from(self.bits())));
        TyKind::Primitive(Primitive::Lang(Lang::Float), vec![bits]).intern()
    }
}
