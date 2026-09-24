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

    /// The item type of a `lang.Bounds` type: the integers its ranges hold.
    #[must_use]
    pub fn bounds(&self) -> Option<Ty> {
        match self {
            Self::Primitive(Primitive::Lang(Lang::Bounds), args) => match args.as_slice() {
                [Value::Type(items)] => Some(*items),
                _ => None,
            },
            _ => None,
        }
    }

    /// The item type of a slice type.
    #[must_use]
    pub fn slice_items(&self) -> Option<Ty> {
        match self {
            Self::Primitive(Primitive::Mem(Mem::Slice), args) => match args.as_slice() {
                [Value::Type(elem)] => Some(*elem),
                _ => None,
            },
            _ => None,
        }
    }

    /// The item type of an array or slice type.
    #[must_use]
    pub fn items(&self) -> Option<Ty> {
        self.slice_items().or_else(|| self.as_array().map(|(_, elem)| elem))
    }

    /// `lang.Int(range)`.
    #[must_use]
    pub fn int(range: &Interval) -> Ty {
        Self::Primitive(Primitive::Lang(Lang::Int), vec![Value::Range(Some(range.clone()))]).intern()
    }

    /// Whether a value of type `found` is accepted where this type is expected: integers whose
    /// range lies within (spec 04, rule 2), tuples, arrays and bounds whose items are, bounds
    /// where a `lang.Range` is expected, a borrow of an array where a borrow of a slice of its
    /// items is (spec 02), or the type itself.
    #[must_use]
    pub fn accepts(&self, found: &Self) -> bool {
        if let (Some(expected), Some(found)) = (self.interval(), found.interval()) {
            return expected.contains(&found);
        }
        if let Some(found_items) = found.bounds() {
            return match self.bounds() {
                Some(items) => items.accepts(&found_items),
                None => *self == Self::Primitive(Primitive::Lang(Lang::Range), Vec::new()),
            };
        }
        // A borrow of an array is a borrow of its items as a slice, in the same mode.
        if let (Some((mode, _, expected)), Some((found_mode, _, found))) =
            (self.borrowing(), found.borrowing())
            && mode == found_mode
            && let (Some(items), Some((_, found_items))) = (expected.slice_items(), found.as_array())
        {
            return items.accepts(&found_items);
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
        self.borrowing().map(|(_, p, ty)| (p, ty))
    }

    /// The type a borrow or a raw pointer points to: what `.*` reads.
    #[must_use]
    pub fn pointee(&self) -> Option<Ty> {
        match self {
            Self::Primitive(Primitive::Mem(Mem::Ptr), args) => match args.as_slice() {
                [Value::Type(ty)] => Some(*ty),
                _ => None,
            },
            _ => self.borrowed(),
        }
    }

    /// For a borrow type: the word after `&` that writes it (the table `borrows`), its
    /// constructor and the type borrowed.
    #[must_use]
    pub fn borrowing(&self) -> Option<(&'static str, Primitive, Ty)> {
        let Self::Primitive(p, args) = self else { return None };
        let [Value::Type(ty)] = args.as_slice() else { return None };
        BORROWS.iter().find_map(|(mode, item)| {
            let (ns, name) = item.rsplit_once(PATH_SEPARATOR)?;
            (ns == p.namespace() && name == p.name()).then_some((*mode, *p, *ty))
        })
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
