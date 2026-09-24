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

use overt_grammar::{BORROWS, PATH_SEPARATOR};
use overt_intern::{Interned, Pool, Symbol};

use crate::Primitive;
use crate::eval::Instance;
use crate::int::{Integer, Interval};
use crate::item::ExprRef;
use crate::primitives::lang::Lang;
use crate::primitives::mem::Mem;

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

    /// `lang.mem.Array(len, elem)`: `[len]elem`.
    #[must_use]
    pub fn array(len: Integer, elem: Ty) -> Ty {
        Self::Primitive(Primitive::Mem(Mem::Array), vec![Value::Int(len), Value::Type(elem)]).intern()
    }

    /// `lang.mem.Slice(elem)`: `[elem]`.
    #[must_use]
    pub fn slice(elem: Ty) -> Ty {
        Self::Primitive(Primitive::Mem(Mem::Slice), vec![Value::Type(elem)]).intern()
    }

    /// The length and item type of an array type.
    #[must_use]
    pub fn as_array(&self) -> Option<(&Integer, Ty)> {
        match self {
            Self::Primitive(Primitive::Mem(Mem::Array), args) => match args.as_slice() {
                [Value::Int(len), Value::Type(elem)] => Some((len, *elem)),
                _ => None,
            },
            _ => None,
        }
    }

    /// The item type of an array or slice type.
    #[must_use]
    pub fn items(&self) -> Option<Ty> {
        match self {
            Self::Primitive(Primitive::Mem(Mem::Slice), args) => match args.as_slice() {
                [Value::Type(elem)] => Some(*elem),
                _ => None,
            },
            _ => self.as_array().map(|(_, elem)| elem),
        }
    }

    /// `lang.Int(range)`.
    #[must_use]
    pub fn int(range: &Interval) -> Ty {
        Self::Primitive(Primitive::Lang(Lang::Int), vec![Value::Range(Some(range.clone()))]).intern()
    }

    /// Whether a value of type `found` is accepted where this type is expected: integers whose
    /// range lies within (spec 04, rule 2), tuples and arrays whose items are, or the type itself.
    #[must_use]
    pub fn accepts(&self, found: &Self) -> bool {
        if let (Some(expected), Some(found)) = (self.interval(), found.interval()) {
            return expected.contains(&found);
        }
        match (self, found) {
            (Self::Tuple(expected), Self::Tuple(found)) => {
                expected.len() == found.len() && expected.iter().zip(found).all(|(e, f)| e.accepts(f))
            }
            _ => match (self.as_array(), found.as_array()) {
                (Some((len, elem)), Some((found_len, found_elem))) => {
                    len == found_len && elem.accepts(&found_elem)
                }
                _ => self == found,
            },
        }
    }

    /// The type a borrow type borrows: `T` for `&T`, `&mut T` and the other modes of the table
    /// `borrows` (appendix B), which have no namespace of their own (spec 01, section 4).
    #[must_use]
    pub fn borrowed(&self) -> Option<Ty> {
        self.borrow().map(|(_, ty)| ty)
    }

    /// The borrow type constructor and the type borrowed, for a borrow type.
    #[must_use]
    pub fn borrow(&self) -> Option<(Primitive, Ty)> {
        let Self::Primitive(p, args) = self else { return None };
        let [Value::Type(ty)] = args.as_slice() else { return None };
        let borrow = BORROWS.iter().filter_map(|(_, item)| item.rsplit_once(PATH_SEPARATOR));
        borrow.into_iter().any(|(ns, name)| ns == p.namespace() && name == p.name()).then_some((*p, *ty))
    }

    /// The type values of both `self` and `other` have: the hull of two integer ranges, item by
    /// item for tuples and arrays, or the type itself; `None` when they have none in common.
    #[must_use]
    pub fn join(&self, other: &Self) -> Option<Ty> {
        if let (Some(a), Some(b)) = (self.interval(), other.interval()) {
            return Some(Self::int(&a.hull(&b)));
        }
        match (self, other) {
            (Self::Tuple(a), Self::Tuple(b)) if a.len() == b.len() => {
                let items = a.iter().zip(b).map(|(a, b)| a.join(b)).collect::<Option<_>>()?;
                Some(Self::Tuple(items).intern())
            }
            _ => match (self.as_array(), other.as_array()) {
                (Some((len, elem)), Some((other_len, other_elem))) if len == other_len => {
                    Some(Self::array(len.clone(), elem.join(&other_elem)?))
                }
                _ => (self == other).then(|| self.clone().intern()),
            },
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
