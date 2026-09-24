//! Markers of function types (spec 07, section 4): the properties a function type carries, each
//! a `lang` attribute with its arguments, such as `lang.safety.unsafe` or `lang.stack.max(512)`.
//! The table `fn_markers` names them and says how a function carrying one is accepted where a
//! function type is expected; the table `markers` gives the prefixes that put one.

use vun_grammar::{FN_MARKERS, MARKERS, PATH_SEPARATOR, Relation, TokenKind};

use super::Value;
use crate::Primitive;
use crate::primitives::panic::Panic;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Marker {
    pub item: Primitive,
    /// The attribute's arguments: `512` of `stack.max(512)`.
    pub args: Vec<Value>,
}

impl Marker {
    /// The marker the prefix `token` puts on a function type (the table `markers`).
    #[must_use]
    pub fn of_prefix(token: TokenKind) -> Option<Self> {
        let (_, item) = MARKERS.iter().find(|(t, _)| *t == token)?;
        Some(Self { item: Self::primitive(item)?, args: Vec::new() })
    }

    /// The primitive at the path `item` of `lang`.
    fn primitive(item: &str) -> Option<Primitive> {
        let (ns, name) = item.rsplit_once(PATH_SEPARATOR)?;
        Primitive::ALL.iter().copied().find(|p| p.namespace() == ns && p.name() == name)
    }

    /// The attributes that are markers, with their relations (the table `fn_markers`).
    pub fn kinds() -> impl Iterator<Item = (Primitive, Relation)> {
        FN_MARKERS.iter().filter_map(|(item, relation)| Some((Self::primitive(item)?, *relation)))
    }

    /// Whether `markers` promise that the function never traps (`#[panic.never]`).
    #[must_use]
    pub fn never_traps(markers: &[Self]) -> bool {
        markers.iter().any(|m| m.item == Primitive::Panic(Panic::never))
    }

    /// How a function carrying this marker is accepted where a function type is expected.
    #[must_use]
    pub fn relation(&self) -> Option<Relation> {
        Self::kinds().find(|(p, _)| *p == self.item).map(|(_, r)| r)
    }

    /// Where this marker falls among the others: the order of the table `fn_markers`, so that a
    /// set of markers has one spelling.
    fn rank(&self) -> Option<usize> {
        Self::kinds().position(|(p, _)| p == self.item)
    }

    /// Adds `marker` to `markers`, keeping them in order and each item once.
    pub fn insert(markers: &mut Vec<Self>, marker: Self) {
        if !markers.iter().any(|m| m.item == marker.item) {
            markers.push(marker);
            markers.sort_by_key(Self::rank);
        }
    }

    /// Whether a function carrying `found` may be used where a type carrying `expected` is.
    #[must_use]
    pub fn accepts(expected: &[Self], found: &[Self]) -> bool {
        let with = |markers: &[Self], relation| {
            markers.iter().filter(|m| m.relation() == Some(relation)).cloned().collect::<Vec<_>>()
        };
        let within = |a: &[Self], b: &[Self]| a.iter().all(|m| b.contains(m));
        let duties = within(&with(found, Relation::Duty), &with(expected, Relation::Duty));
        let guarantees = within(&with(expected, Relation::Guarantee), &with(found, Relation::Guarantee));
        let conventions = with(expected, Relation::Identity) == with(found, Relation::Identity);
        let bounds = with(expected, Relation::Bound).iter().all(|limit| {
            found.iter().find(|m| m.item == limit.item).is_some_and(|m| {
                match (m.args.first(), limit.args.first()) {
                    (Some(Value::Int(have)), Some(Value::Int(most))) => have <= most,
                    _ => m == limit,
                }
            })
        });
        duties && guarantees && conventions && bounds
    }

    /// The marker as a value, which a function type holds: the item, then its arguments.
    #[must_use]
    pub fn value(&self) -> Value {
        Value::Tuple(std::iter::once(Value::Primitive(self.item)).chain(self.args.iter().cloned()).collect())
    }

    /// The marker a value of a function type holds.
    #[must_use]
    pub fn from_value(value: &Value) -> Option<Self> {
        let Value::Tuple(parts) = value else { return None };
        let (Value::Primitive(item), args) = parts.split_first()? else { return None };
        Some(Self { item: *item, args: args.to_vec() })
    }
}
