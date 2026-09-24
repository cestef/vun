//! Compile-time values (spec 03, section 7): what evaluation produces and what types are applied
//! to. Every value hashes by content, so types built from values do too.

use vun_intern::Symbol;
use vun_origin::OriginId;

use vun_hir::db::Namespace;

use super::{Ty, TyKind};
use crate::Primitive;
use crate::eval::{Code, Func, Suspended};
use crate::int::{Extended, Integer, Interval};
use crate::item::DeclRef;
use crate::primitives::meta::Meta;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Value {
    Int(Integer),
    /// A `lang.Range`: the integers it holds, `None` when it holds none. `a..b` and `a..=b - 1`
    /// are one value.
    Range(Option<Interval>),
    Type(Ty),
    Tuple(Vec<Value>),
    Array(Vec<Value>),
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
    Fn(Func),
    /// A variant with a payload named as a value, `E.fs`: the function from its payload to the
    /// variant.
    Constructor {
        ty: Ty,
        name: Symbol,
    },
    /// A closure: its function, and the values of its captures in the order written, passed
    /// before the arguments.
    Closure {
        f: Func,
        captures: Vec<Value>,
    },
    /// A declaration the linker provides (`name: T = _;` outside `lang`): its link symbol, whose
    /// value exists at run time only.
    Linked(DeclRef),
    /// A coroutine's frame (spec 06).
    Frame(Box<Suspended>),
    /// Code, not its value (spec 03, section 4).
    Code(Code),
    /// Where something is written: a `lang.meta.Origin`.
    Origin(OriginId),
    /// A bare `.name` where no type is expected: a `lang.meta.Name`.
    Name(Symbol),
    /// `lang.interface(f)`: the function `f` from a type to its requirement struct (spec 02,
    /// section 3).
    Interface(Func),
}

impl Value {
    /// A string at compile time: its bytes (decision 82).
    #[must_use]
    pub fn string(text: &str) -> Self {
        Self::Array(text.bytes().map(|b| Self::Int(Integer::from(i128::from(b)))).collect())
    }

    /// The text an array of bytes holds, as `Self::string` makes it.
    #[must_use]
    pub fn text(&self) -> Option<String> {
        let Self::Array(bytes) = self else { return None };
        let bytes = bytes.iter().map(|b| match b {
            Self::Int(n) => n.to_i128().and_then(|n| u8::try_from(n).ok()),
            _ => None,
        });
        String::from_utf8(bytes.collect::<Option<_>>()?).ok()
    }

    /// The items a `$for` steps through: those of an array, or the integers of a finite range.
    #[must_use]
    pub fn items(self) -> Option<Vec<Self>> {
        match self {
            Self::Array(items) => Some(items),
            Self::Range(None) => Some(Vec::new()),
            Self::Range(Some(range)) => {
                let (Extended::Finite(lo), Extended::Finite(hi)) = (range.lo(), range.hi()) else {
                    return None;
                };
                let (lo, hi) = (lo.to_i128()?, hi.to_i128()?);
                Some((lo..=hi).map(|n| Self::Int(Integer::from(n))).collect())
            }
            _ => None,
        }
    }

    /// The type of this value, for a value of a type with methods: an integer's is the type of
    /// that integer alone, since every integer type holding it has the same impls.
    #[must_use]
    pub fn ty(&self) -> Option<Ty> {
        match self {
            Self::Int(n) => Some(TyKind::int(&Interval::singleton(n.clone()))),
            Self::Struct { ty, .. } | Self::Variant { ty, .. } => Some(*ty),
            Self::Name(_) => Some(TyKind::primitive(Primitive::Meta(Meta::Name))),
            Self::Tuple(items) => {
                Some(TyKind::Tuple(items.iter().map(Self::ty).collect::<Option<_>>()?).intern())
            }
            _ => None,
        }
    }

    /// The type this value denotes where a type is expected: a type, a primitive that takes no
    /// arguments, such as `lang.f32`, a tuple of types, such as `(u8, bool)` or `()`, or an array
    /// holding one type, the slice type `[T]`: type syntax mirrors value syntax (spec 01).
    #[must_use]
    pub fn as_type(&self) -> Option<Ty> {
        match self {
            Self::Type(ty) => Some(*ty),
            Self::Primitive(p) => Some(TyKind::primitive(*p)),
            Self::Tuple(items) => {
                Some(TyKind::Tuple(items.iter().map(Self::as_type).collect::<Option<_>>()?).intern())
            }
            Self::Array(items) => match items.as_slice() {
                [item] => Some(TyKind::slice(item.as_type()?)),
                _ => None,
            },
            _ => None,
        }
    }
}
