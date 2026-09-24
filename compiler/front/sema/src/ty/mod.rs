//! Types (spec 02): compile-time values of type `type`, interned so that equal types are one
//! pointer and hash by content in every process.
//!
//! Every type is a primitive of `lang` applied to its compile-time arguments (`lang.Int(0..=255)`,
//! `lang.mem.Ref(T)`, `lang.f32`), a `struct` or `enum` literal, a tuple or an array. Syntax that
//! names a type desugars into these (appendix B): `&T` is `lang.mem.Ref(T)`, `?T` is the enum
//! `lang.Option(T)` evaluates to.

mod display;
mod signature;
#[cfg(test)]
mod tests;
mod value;

use overt_intern::{Interned, Pool, Symbol};

use crate::Primitive;
use crate::eval::Instance;
use crate::int::{Integer, Interval};
use crate::item::ExprRef;
use crate::primitives::lang::Lang;

pub use signature::Signature;
pub use value::Value;

/// A type.
pub type Ty = Interned<TyKind>;

static TYPES: Pool<TyKind> = Pool::new();

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TyKind {
    /// A primitive type or type constructor, applied to its compile-time arguments. An integer
    /// type's argument is the set its range holds, so `Int(0..256)` and `Int(0..=255)` are one
    /// type.
    Primitive(Primitive, Vec<Value>),
    /// A `struct` or `enum` literal (decision 28).
    Nominal(Nominal),
    Tuple(Vec<Ty>),
    Array {
        len: Integer,
        elem: Ty,
    },
}

/// The identity of a `struct` or `enum` literal: where it is written and the compile-time call
/// that evaluated it (decision 28). Its members are looked up from the literal.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Nominal {
    pub literal: ExprRef,
    /// `None` for a literal evaluated outside any call.
    pub instance: Option<Instance>,
    pub kind: NominalKind,
    /// The declaration the literal is written in, which diagnostics name the type by.
    pub name: Option<Symbol>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NominalKind {
    Struct,
    Enum,
}

impl TyKind {
    #[must_use]
    pub fn intern(self) -> Ty {
        TYPES.intern(self)
    }

    /// A primitive type that takes no arguments, such as `lang.type` or `lang.f32`.
    #[must_use]
    pub fn primitive(primitive: Primitive) -> Ty {
        Self::Primitive(primitive, Vec::new()).intern()
    }

    /// `lang.Int(range)`.
    #[must_use]
    pub fn int(range: &Interval) -> Ty {
        Self::Primitive(Primitive::Lang(Lang::Int), vec![Value::Range(Some(range.clone()))]).intern()
    }

    /// Whether a value of type `found` is accepted where this type is expected: integers whose
    /// range lies within (spec 04, rule 2), or the type itself.
    #[must_use]
    pub fn accepts(&self, found: &Self) -> bool {
        match (self.interval(), found.interval()) {
            (Some(expected), Some(found)) => expected.contains(&found),
            _ => self == found,
        }
    }

    /// The range of an integer type.
    #[must_use]
    pub fn interval(&self) -> Option<Interval> {
        match self {
            Self::Primitive(Primitive::Lang(Lang::Int), args) => match args.as_slice() {
                [Value::Range(range)] => range.clone(),
                _ => None,
            },
            _ => None,
        }
    }
}
