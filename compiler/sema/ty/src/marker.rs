//! Markers of function types (spec 07, section 4): the properties a function type carries, each
//! a `lang` attribute with its arguments, such as `lang.safety.unsafe` or `lang.stack.max(512)`.
//! The table `fn_markers` names them and says how a function carrying one is accepted where a
//! function type is expected; the table `markers` gives the prefixes that put one.

use vun_codec::{Decode, Encode};
use vun_desugar::{FN_MARKERS, MARKERS, Relation};
use vun_grammar::TokenKind;
use vun_primitives::Primitive;
use vun_primitives::abi::Abi;
use vun_primitives::panic::Panic;
use vun_primitives::stack::Stack;

use super::Value;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Marker {
    pub item: Primitive,
    /// The attribute's arguments: `512` of `stack.max(512)`.
    pub args: Vec<Value>,
}

impl Marker {
    /// The marker the prefix `token` puts on a function type (the table `markers`).
    #[must_use]
    pub fn of_prefix(token: TokenKind) -> Option<Self> {
        let row = MARKERS.iter().find(|row| row.token == token)?;
        Some(Self { item: Primitive::at(row.item)?, args: Vec::new() })
    }

    /// The attributes that are markers, with their relations (the table `fn_markers`).
    pub fn kinds() -> impl Iterator<Item = (Primitive, Relation)> {
        FN_MARKERS.iter().filter_map(|row| Some((Primitive::at(row.item)?, row.relation)))
    }

    /// The calling convention `markers` name (`#[abi.c]`), if any.
    #[must_use]
    pub fn abi(markers: &[Self]) -> Option<Abi> {
        markers.iter().find_map(|m| match m.item {
            Primitive::Abi(abi) => Some(abi),
            _ => None,
        })
    }

    /// The most bytes of stack a call takes, as `markers` bound it (`#[stack.max(n)]`).
    #[must_use]
    pub fn stack_max(markers: &[Self]) -> Option<u64> {
        let marker = markers.iter().find(|m| m.item == Primitive::Stack(Stack::max))?;
        let [Value::Int(n)] = marker.args.as_slice() else { return None };
        u64::try_from(n.to_i128()?).ok()
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

/// As an attribute is written: `#[lang.stack.max(512)]`.
impl std::fmt::Display for Marker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#[{}", self.item.item())?;
        if !self.args.is_empty() {
            f.write_str("(")?;
            for (i, arg) in self.args.iter().enumerate() {
                if i > 0 {
                    f.write_str(", ")?;
                }
                write!(f, "{arg}")?;
            }
            f.write_str(")")?;
        }
        f.write_str("]")
    }
}
