//! How types relate: which values a type accepts where another is expected, and the type values
//! of two types both have.

use vun_desugar::{Access, Extent};
use vun_primitives::Primitive;
use vun_primitives::lang::Lang;

use super::{Nominal, NominalKind, Ty, TyKind, Value};
use crate::call::Instantiation;

impl TyKind {
    /// Whether this type has no value, such as `never`: what has it never finishes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Nominal(n) if n.kind == NominalKind::Empty)
    }

    /// Whether a value of type `found` is accepted where this type is expected: integers whose
    /// range lies within (spec 04, rule 2), tuples, arrays and bounds whose items are, bounds
    /// where a `lang.Range` is expected, a borrow of an array where a borrow of a slice of its
    /// items is (spec 02), code of an accepted value, a type made with `never` where the same
    /// constructor is expected, or the type itself. An empty type such as `never` is accepted
    /// anywhere: it has no value to be wrong, so a trap or a `return` fits any arm. A function
    /// with `$` parameters is accepted where a function type is: the call instantiating it
    /// decides, as a generic body is checked per instantiation.
    #[must_use]
    pub fn accepts(&self, found: &Self) -> bool {
        if found.is_empty() || matches!(found, Self::Generic(_)) && self.signature().is_some() {
            return true;
        }
        if let (Some(expected), Some(found)) = (self.interval(), found.interval()) {
            return expected.contains(&found);
        }
        if let Some(found_items) = found.bounds() {
            return match self.bounds() {
                Some(items) => items.accepts(&found_items),
                None => *self == Self::Primitive(Primitive::Lang(Lang::Range), Vec::new()),
            };
        }
        // A borrow serving where another is expected is accepted there: one of the same
        // transition leaving others no more, so `&mut` where `&` is expected; a `&temp`
        // parameter takes an ordinary borrow, lent for the call only, and a `&static` one goes
        // wherever a borrow does; only where `&static` is written is it required (spec 05,
        // section 4).
        if let (Some((mode, p, _)), Some((found_mode, _, pointee))) = (self.borrowing(), found.borrowing())
            && mode != found_mode
            && found_mode.serves(mode)
            && self.extent() != Some(Extent::Static)
        {
            return self.accepts(&Self::Primitive(p, vec![Value::Type(pointee)]).intern());
        }
        // A borrow of an array is a borrow of its items as a slice, in the same mode; only a
        // shared one may see them as a wider item type, since an exclusive one writes through.
        if let (Some((mode, _, expected)), Some((found_mode, _, found))) =
            (self.borrowing(), found.borrowing())
            && mode == found_mode
            && let (Some(items), Some((_, found_items))) = (expected.slice_items(), found.as_array())
        {
            return if mode.access() == Access::Shared {
                items.accepts(&found_items)
            } else {
                items == found_items
            };
        }
        // A function whose markers the expected type has too (spec 07, section 4).
        if let (Some(expected), Some(found)) = (self.signature(), found.signature()) {
            return expected.accepts(&found);
        }
        // Code of a value is accepted where code of a type accepting it is.
        if let (Some(expected), Some(found)) = (self.quoted(), found.quoted()) {
            return expected.accepts(&found);
        }
        match (self, found) {
            (Self::Record(expected), Self::Record(found)) => {
                expected.len() == found.len()
                    && expected.iter().zip(found).all(|((n, e), (m, f))| n == m && e.accepts(f))
            }
            (Self::Tuple(expected), Self::Tuple(found)) => {
                expected.len() == found.len() && expected.iter().zip(found).all(|(e, f)| e.accepts(f))
            }
            _ => match (self.as_array(), found.as_array()) {
                (Some((len, elem)), Some((found_len, found_elem))) => {
                    len == found_len && elem.accepts(&found_elem)
                }
                _ => self == found || self.made_with_empty(found),
            },
        }
    }

    /// The array a temporary `found` is stored as where a borrow of this slice type is expected to
    /// reach it: of as many items as `found` holds, each of this slice's item type, which accepts
    /// the items of `found`. A slice is a view of the items of the array under it, so they are
    /// laid out as the slice's items are.
    #[must_use]
    pub fn stored_as(&self, found: &Self) -> Option<Ty> {
        let (wanted, (len, item)) = (self.slice_items()?, found.as_array()?);
        wanted.accepts(&item).then(|| Self::array(len.clone(), wanted))
    }

    /// Whether `found` is made by the same constructor as this type with an empty type, such as
    /// `lang.never`, in place of some arguments: `Result(never, E)` where `Result(T, E)` is
    /// expected. Its only values are those whose payloads avoid the empty type, which are values
    /// of this type too.
    fn made_with_empty(&self, found: &Self) -> bool {
        let (Self::Nominal(expected), Self::Nominal(found)) = (self, found) else { return false };
        let (Some(call), Some(found_call)) = (&expected.instance, &found.instance) else { return false };
        let empty = |v: &Option<Value>| v.as_ref().and_then(Value::as_type).is_some_and(|t| t.is_empty());
        expected.literal == found.literal
            && call.f == found_call.f
            && call.args.len() == found_call.args.len()
            && call.args.iter().zip(&found_call.args).all(|(a, b)| a == b || empty(b))
    }

    /// The type values of both `self` and `other` have: the hull of two integer ranges, item by
    /// item for tuples and arrays, or the one that accepts the other; `None` when they have none in common.
    #[must_use]
    pub fn join(&self, other: &Self) -> Option<Ty> {
        if let (Some(a), Some(b)) = (self.interval(), other.interval()) {
            return Some(Self::int(&a.hull(&b)));
        }
        // Borrows of one transition meet in the access both serve, and in the shorter extent, a
        // `&static` one as the other; borrows of arrays or slices meet as a borrow of a slice of
        // their items' join, or, for an exclusive borrow, which writes through, of their one item
        // type, so that any number of them join in any order.
        if let (Some((mode, p, a)), Some((other_mode, q, b))) = (self.borrowing(), other.borrowing())
            && (mode.serves(other_mode) || other_mode.serves(mode))
            && (a != b || mode != other_mode)
        {
            // The one the other serves, else the one of the shorter extent.
            let (mode, p) = match (other_mode.serves(mode), mode.serves(other_mode)) {
                (true, false) => (mode, p),
                (false, true) => (other_mode, q),
                _ if self.extent() == Some(Extent::Static) => (other_mode, q),
                _ => (mode, p),
            };
            let pointee = match (a.items(), b.items()) {
                _ if a == b => a,
                (Some(a), Some(b)) if mode.access() == Access::Shared => Self::slice(a.join(&b)?),
                (Some(a), Some(b)) if a == b => Self::slice(a),
                _ => return None,
            };
            return Some(Self::Primitive(p, vec![Value::Type(pointee)]).intern());
        }
        match (self, other) {
            (Self::Record(a), Self::Record(b)) if a.len() == b.len() => {
                let fields = a
                    .iter()
                    .zip(b)
                    .map(|((n, x), (m, y))| (n == m).then(|| Some((*n, x.join(y)?))).flatten())
                    .collect::<Option<_>>()?;
                Some(Self::Record(fields).intern())
            }
            (Self::Tuple(a), Self::Tuple(b)) if a.len() == b.len() => {
                let items = a.iter().zip(b).map(|(a, b)| a.join(b)).collect::<Option<_>>()?;
                Some(Self::Tuple(items).intern())
            }
            _ => match (self.as_array(), other.as_array()) {
                (Some((len, elem)), Some((other_len, other_elem))) if len == other_len => {
                    Some(Self::array(len.clone(), elem.join(&other_elem)?))
                }
                _ if self.accepts(other) => Some(self.clone().intern()),
                _ => other.accepts(self).then(|| other.clone().intern()),
            },
        }
    }
}

impl TyKind {
    /// The arguments `constructor` made this type from: none when the constructor is the type
    /// itself, those of `lang.Int(r)` for `lang.Int`, those of the call for a function returning
    /// a type.
    #[must_use]
    pub fn made_by(&self, constructor: &Value) -> Option<Vec<Value>> {
        match (constructor, self) {
            (Value::Type(ty), _) => (**ty == *self).then(Vec::new),
            (Value::Primitive(p), Self::Primitive(q, args)) => (p == q).then(|| args.clone()),
            (Value::Fn(f), Self::Nominal(Nominal { instance: Some(made), .. })) => {
                made.call().filter(|call| call.f == *f).map(|call| call.args)
            }
            _ => None,
        }
    }

    /// The arguments of the interface made by `constructor` that this type implements, the one
    /// it is declared with: a `$` parameter's constraint, such as `Call((), ())`, or the
    /// interface a frame's function declares, such as `Coroutine(u32, ())`, and the call it is
    /// written as, such as `Iter(u32)`.
    #[must_use]
    pub fn implements(&self, constructor: &Value) -> Option<Vec<Value>> {
        let Value::Fn(f) = constructor else { return None };
        let interface = self.interface()?;
        let written = match self {
            Self::Frame(frame) => frame.written.as_ref(),
            _ => None,
        };
        let made =
            interface.within.as_deref().and_then(Instantiation::call).into_iter().chain(written.cloned());
        made.into_iter().find(|made| made.f == *f).map(|made| made.args)
    }
}
