//! How types relate: which values a type accepts where another is expected, and the type values
//! of two types both have.

use vun_desugar::{Access, Extent};

use super::{NominalKind, Ty, TyKind, Value};
use crate::Primitive;
use crate::primitives::lang::Lang;

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
    /// anywhere: it has no value to be wrong, so a trap or a `return` fits any arm.
    #[must_use]
    pub fn accepts(&self, found: &Self) -> bool {
        if found.is_empty() {
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
        // Borrows of one transition and access are accepted for each other: a `&temp` parameter
        // takes an ordinary borrow, lent for the call only, and a `&static` one goes wherever a
        // borrow does; only where `&static` is written is it required (spec 05, section 4).
        if let (Some((mode, p, _)), Some((found_mode, _, pointee))) = (self.borrowing(), found.borrowing())
            && mode != found_mode
            && mode.alike(found_mode)
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

    /// Whether `found` is made by the same constructor as this type with an empty type, such as
    /// `lang.never`, in place of some arguments: `Result(never, E)` where `Result(T, E)` is
    /// expected. Its only values are those whose payloads avoid the empty type, which are values
    /// of this type too.
    fn made_with_empty(&self, found: &Self) -> bool {
        let (Self::Nominal(expected), Self::Nominal(found)) = (self, found) else { return false };
        let (Some(call), Some(found_call)) = (&expected.instance, &found.instance) else { return false };
        let empty = |v: &Value| v.as_type().is_some_and(|t| t.is_empty());
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
        // Borrows of one transition and access meet in the shorter extent, a `&static` one as
        // the other; borrows of arrays meet as a borrow of a slice of their items' join, or, for
        // an exclusive borrow, which writes through, of their one item type.
        if let (Some((mode, p, a)), Some((other_mode, q, b))) = (self.borrowing(), other.borrowing())
            && mode.alike(other_mode)
            && (a != b || mode != other_mode)
        {
            let p = if self.extent() == Some(Extent::Static) { q } else { p };
            let pointee = match (a.as_array(), b.as_array()) {
                _ if a == b => a,
                (Some((_, a)), Some((_, b))) if mode.access() == Access::Shared => Self::slice(a.join(&b)?),
                (Some((_, a)), Some((_, b))) if a == b => Self::slice(a),
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
