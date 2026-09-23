//! Compile-time values (spec 03, section 7): what evaluation produces and what types are applied
//! to. Every value hashes by content, so types built from values do too.

use overt_intern::Symbol;

use overt_hir::db::Namespace;

use super::{Ty, TyKind};
use crate::Primitive;
use crate::eval::FnRef;
use crate::int::{Integer, Interval};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Value {
    Int(Integer),
    /// A `lang.Range`: the integers it holds, `None` when it holds none. `a..b` and `a..=b - 1`
    /// are one value.
    Range(Option<Interval>),
    Type(Ty),
    Tuple(Vec<Value>),
    /// A variant of an enum type, with its payload.
    Variant {
        ty: Ty,
        name: Symbol,
        payload: Vec<Value>,
    },
    /// A value of a struct type, its fields in declaration order.
    Struct {
        ty: Ty,
        fields: Vec<(Symbol, Value)>,
    },
    /// A primitive function or type constructor itself, such as `lang.Int`.
    Primitive(Primitive),
    /// A file or directory: a struct whose members are its declarations (spec 01, section 8).
    Namespace(Namespace),
    /// A function literal.
    Fn(FnRef),
}

impl Value {
    /// The type this value denotes where a type is expected: a type, a primitive that takes no
    /// arguments, such as `lang.f32`, or a tuple of types, such as `(u8, bool)` or `()`.
    #[must_use]
    pub fn as_type(&self) -> Option<Ty> {
        match self {
            Self::Type(ty) => Some(*ty),
            Self::Primitive(p) => Some(TyKind::primitive(*p)),
            Self::Tuple(items) => {
                Some(TyKind::Tuple(items.iter().map(Self::as_type).collect::<Option<_>>()?).intern())
            }
            _ => None,
        }
    }
}
