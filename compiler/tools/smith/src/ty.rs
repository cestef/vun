//! The types of the model: what the generator tracks of every form it makes, and what prints as
//! the type of a binding or a parameter.

use crate::float::Format;
use crate::int::Range;
use crate::ir::TypeId;

/// A type the model knows. Aggregates hold their parts' types, so what widens in a part widens
/// the whole (spec 04, rule 2).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Ty {
    Int(Range),
    Bool,
    Float(Format),
    /// A closure, by the number of the expression that made it, and what it takes and gives.
    Closure(usize, Vec<Ty>, Box<Ty>),
    /// A borrow of a value: mutable (`&mut`) or shared (`&`).
    Ref(bool, Box<Ty>),
    Unit,
    /// The `n`th type parameter of the generic function being made.
    Param(usize),
    Tuple(Vec<Ty>),
    Array(usize, Box<Ty>),
    /// A struct declared by the program, and the types of its fields.
    Struct(TypeId, Vec<Ty>),
    /// An enum declared by the program, and the types each variant holds.
    Enum(Nominal, Vec<Vec<Ty>>),
}

/// Which enum a type is: one the program declares, or `lang.Option` or `lang.Result` of some
/// types, which are distinct types for distinct arguments.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Nominal {
    Declared(TypeId),
    Option,
    Result,
}

impl Nominal {
    /// The name of the `at`th variant.
    #[must_use]
    pub fn variant(self, at: usize) -> String {
        match self {
            Self::Declared(_) => format!("c{at}"),
            Self::Option => ["none", "some"][at].to_owned(),
            Self::Result => ["ok", "err"][at].to_owned(),
        }
    }
}

impl Ty {
    /// The type with each parameter replaced by the argument given for it.
    #[must_use]
    pub fn subst(&self, args: &[Self]) -> Self {
        let each = |parts: &[Self]| parts.iter().map(|p| p.subst(args)).collect::<Vec<_>>();
        match self {
            Self::Param(at) => args[*at].clone(),
            Self::Closure(id, params, result) => {
                Self::Closure(*id, each(params), Box::new(result.subst(args)))
            }
            Self::Ref(mutable, inner) => Self::Ref(*mutable, Box::new(inner.subst(args))),
            Self::Tuple(parts) => Self::Tuple(each(parts)),
            Self::Array(len, item) => Self::Array(*len, Box::new(item.subst(args))),
            Self::Struct(id, parts) => Self::Struct(*id, each(parts)),
            Self::Enum(nominal, variants) => Self::Enum(*nominal, variants.iter().map(|v| each(v)).collect()),
            Self::Int(_) | Self::Bool | Self::Float(_) | Self::Unit => self.clone(),
        }
    }

    /// Whether a type parameter is anywhere in it.
    #[must_use]
    pub fn has_param(&self) -> bool {
        match self {
            Self::Param(_) => true,
            Self::Closure(_, params, result) => params.iter().any(Self::has_param) || result.has_param(),
            Self::Tuple(parts) | Self::Struct(_, parts) => parts.iter().any(Self::has_param),
            Self::Enum(_, variants) => variants.iter().flatten().any(Self::has_param),
            Self::Array(_, item) | Self::Ref(_, item) => item.has_param(),
            Self::Int(_) | Self::Bool | Self::Float(_) | Self::Unit => false,
        }
    }

    /// `lang.Option` of `inner`.
    #[must_use]
    pub fn option(inner: Self) -> Self {
        Self::Enum(Nominal::Option, vec![Vec::new(), vec![inner]])
    }

    /// The integer type of `range`.
    #[must_use]
    pub fn int(range: Range) -> Self {
        Self::Int(range)
    }

    /// Whether a value of `actual` is accepted where this type is expected: a range within a
    /// range, parts by part, anything else only itself.
    #[must_use]
    pub fn accepts(&self, actual: &Self) -> bool {
        match (self, actual) {
            (Self::Int(want), Self::Int(have)) => have.within(want),
            (Self::Tuple(want), Self::Tuple(have)) => {
                want.len() == have.len() && want.iter().zip(have).all(|(w, h)| w.accepts(h))
            }
            (Self::Array(n, want), Self::Array(m, have)) => n == m && want.accepts(have),
            _ => self == actual,
        }
    }

    /// The type both of two branches have: the hull of ranges, part by part; none when they have
    /// nothing in common.
    #[must_use]
    pub fn join(&self, other: &Self) -> Option<Self> {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => Some(Self::Int(a.hull(b))),
            (Self::Tuple(a), Self::Tuple(b)) if a.len() == b.len() => {
                a.iter().zip(b).map(|(x, y)| x.join(y)).collect::<Option<_>>().map(Self::Tuple)
            }
            (Self::Array(n, a), Self::Array(m, b)) if n == m => {
                a.join(b).map(|t| Self::Array(*n, Box::new(t)))
            }
            _ if self == other => Some(self.clone()),
            _ => None,
        }
    }

    /// The range, for an integer.
    #[must_use]
    pub fn range(&self) -> Option<&Range> {
        match self {
            Self::Int(range) => Some(range),
            _ => None,
        }
    }

    /// Whether every integer in it is within the widest the compiler admits.
    #[must_use]
    pub fn fits_a_word(&self) -> bool {
        match self {
            Self::Int(range) => range.fits_a_word(),
            Self::Tuple(parts) | Self::Struct(_, parts) => parts.iter().all(Self::fits_a_word),
            Self::Enum(_, variants) => variants.iter().flatten().all(Self::fits_a_word),
            Self::Array(_, item) | Self::Ref(_, item) => item.fits_a_word(),
            _ => true,
        }
    }

    /// Whether a call taking `wanted` can reuse the memory of a caller that received `held`:
    /// every argument that may live in memory has a parameter of its type to take the place of.
    #[must_use]
    pub fn fit_received(wanted: &[Self], held: &[Self]) -> bool {
        let mut free: Vec<&Self> = held.iter().collect();
        wanted.iter().all(|ty| {
            matches!(ty, Self::Int(_) | Self::Bool | Self::Float(_) | Self::Unit)
                || free.iter().position(|h| *h == ty).map(|at| free.swap_remove(at)).is_some()
        })
    }

    /// Whether a borrow or a closure is anywhere in it, which a closure does not capture.
    #[must_use]
    pub fn holds_borrow(&self) -> bool {
        match self {
            Self::Ref(..) | Self::Closure(..) => true,
            Self::Tuple(parts) | Self::Struct(_, parts) => parts.iter().any(Self::holds_borrow),
            Self::Enum(_, variants) => variants.iter().flatten().any(Self::holds_borrow),
            Self::Array(_, item) => item.holds_borrow(),
            _ => false,
        }
    }

    /// How deep the types inside it go.
    #[must_use]
    pub fn depth(&self) -> usize {
        match self {
            Self::Tuple(parts) | Self::Struct(_, parts) => {
                1 + parts.iter().map(Self::depth).max().unwrap_or(0)
            }
            Self::Enum(_, variants) => 1 + variants.iter().flatten().map(Self::depth).max().unwrap_or(0),
            Self::Array(_, item) => 1 + item.depth(),
            Self::Ref(_, inner) => inner.depth(),
            _ => 0,
        }
    }
}
