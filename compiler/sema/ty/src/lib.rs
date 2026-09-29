//! Types (spec 02): compile-time values of type `type`, interned so that equal types are one
//! pointer and hash by content in every process.
//!
//! Every type is a primitive of `lang` applied to its compile-time arguments (`lang.Int(0..=255)`,
//! `lang.mem.Ref(T)`, `lang.f32`), a `struct` or `enum` literal, a tuple or an array. Syntax that
//! names a type desugars into these (appendix B): `&T` is `lang.mem.Ref(T)`, `?T` is the enum
//! `lang.Option(T)` evaluates to.

mod admits;
mod borrow;
pub mod call;
mod display;
pub mod float;
pub mod item;
mod literal;
mod marker;
pub mod ops;
mod relate;
mod sequence;
mod signature;
#[cfg(test)]
mod tests;
pub mod trap;
mod value;

pub use marker::Marker;
pub use signature::Signature;
pub use value::Value;
use vun_codec::{Decode, Encode};
use vun_intern::{Internable, Interned, Pool, Symbol};
use vun_primitives::Primitive;
use vun_primitives::lang::Lang;
use vun_primitives::mem::Mem;
use vun_primitives::meta::Meta;
use vun_sema_int::Interval;

use crate::call::{Func, Instance};
use crate::item::ExprRef;

/// A type.
pub type Ty = Interned<TyKind>;

static TYPES: Pool<TyKind> = Pool::new();

impl Internable for TyKind {
    const POOL: &'static str = "type";

    fn pool() -> &'static Pool<Self> {
        &TYPES
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum TyKind {
    /// A primitive type or type constructor, applied to its compile-time arguments. An integer
    /// type's argument is the set its range holds, so `Int(0..256)` and `Int(0..=255)` are one
    /// type.
    Primitive(Primitive, Vec<Value>),
    /// A `struct` or `enum` literal (decision 28).
    Nominal(Nominal),
    Tuple(Vec<Ty>),
    /// An anonymous literal written where no type is expected (`.{ .a = 1 }`): structural, its
    /// fields sorted by name, so records with the same fields are one type.
    Record(Vec<(Symbol, Ty)>),
    /// A closure (spec 01, section 4): an anonymous struct of its captures, called through
    /// `lang.ops.Call`.
    Closure(Closure),
    /// The frame a call of a coroutine function returns (spec 06, section 1).
    Frame(Box<Frame>),
    /// A `$` parameter constrained by an interface, as the generic body sees it before an
    /// instantiation binds it: its members are the interface's requirements for it (spec 02,
    /// section 3).
    Param(Param),
}

/// A constrained `$` parameter of a function literal, by its name.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Param {
    pub literal: ExprRef,
    pub name: Symbol,
    /// The function from a type to the requirement struct.
    pub interface: Func,
}

/// The type of the frames of one coroutine function: its literal, the call it was evaluated in,
/// the values of its `$` parameters, and the interface it declares, such as `Iter(u32)`, which
/// the frame implements, with the call it is written as. Named `f.Frame` after the declaration of
/// the function.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Frame {
    pub literal: ExprRef,
    pub within: Option<Instance>,
    /// The `$for` bindings of the entries that generated the coroutine function.
    pub staged: std::sync::Arc<[crate::call::Staging]>,
    pub args: Vec<Option<Value>>,
    pub interface: Func,
    /// The call the interface is written as, when it is one: `Iter(u32)`, whose constructor
    /// may be an alias of the one making the interface, `lang.Coroutine`.
    pub written: Option<Instance>,
    pub name: Option<Symbol>,
}

impl Frame {
    /// The instantiation of the coroutine function making frames of this type.
    #[must_use]
    pub fn instantiation(&self) -> crate::call::Instantiation {
        let f = crate::call::Func {
            literal: self.literal.clone(),
            within: self.within.clone().map(std::sync::Arc::new),
            staged: self.staged.clone(),
        };
        crate::call::Instantiation::new(f, self.args.clone())
    }
}

/// The type of a closure literal: its function, which takes the captures before its parameters,
/// the type of each capture by the name of the binder captured, and the signature of the function
/// without them.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Closure {
    pub f: crate::call::Func,
    pub captures: Vec<(Symbol, Ty)>,
    pub signature: Signature,
}

/// The identity of a `struct` or `enum` literal: where it is written and the compile-time call
/// that evaluated it (decision 28). Its members are looked up from the literal.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Nominal {
    pub literal: ExprRef,
    /// `None` for a literal evaluated outside any call.
    pub instance: Option<Instance>,
    pub kind: NominalKind,
    /// The declaration the literal is written in, which diagnostics name the type by.
    pub name: Option<Symbol>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum NominalKind {
    Struct,
    Enum,
    /// An enum without variants, such as `lang.never`: no value has this type.
    Empty,
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

    /// `lang.type`, the type of types.
    #[must_use]
    pub fn of_types() -> Ty {
        Self::primitive(Primitive::Lang(Lang::r#type))
    }

    /// Whether this is a parameter of a generic body or a coroutine's frame, whose impls are
    /// what its requirements say rather than any declared for it.
    #[must_use]
    pub fn is_required(&self) -> bool {
        matches!(self, Self::Param(_) | Self::Frame(_))
    }

    /// Whether the values of this type have members of their own, which the value declares: a
    /// type, an interface, a namespace, or a function making one, such as an interface
    /// constructor. Other values have the members of their type.
    #[must_use]
    pub fn declares_members(&self) -> bool {
        match self {
            Self::Primitive(
                Primitive::Lang(Lang::r#type | Lang::Interface) | Primitive::Meta(Meta::Namespace),
                _,
            ) => true,
            _ => self.signature().is_some_and(|s| s.result.declares_members()),
        }
    }

    /// `lang.meta.Code(ty)`: code whose value has type `ty`.
    #[must_use]
    pub fn code(ty: Ty) -> Ty {
        Self::Primitive(Primitive::Meta(Meta::Code), vec![Value::Type(ty)]).intern()
    }

    /// The type of the value of code of this type: `T` for `Code(T)`.
    #[must_use]
    pub fn quoted(&self) -> Option<Ty> {
        match self {
            Self::Primitive(Primitive::Meta(Meta::Code), args) => match args.as_slice() {
                [Value::Type(ty)] => Some(*ty),
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

    /// `lang.Int(range)`.
    #[must_use]
    pub fn int(range: &Interval) -> Ty {
        Self::Primitive(Primitive::Lang(Lang::Int), vec![Value::Range(Some(range.clone()))]).intern()
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
