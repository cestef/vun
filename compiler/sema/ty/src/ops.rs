//! Compile-time values' parts, repetitions and ranges, and narrowing (spec 04, rule 3). Sema and
//! the interpreter both compute through here; operators are calls (spec 02, section 4).

use std::borrow::Cow;

use vun_codec::{Decode, Encode};
use vun_desugar::Narrowing;
use vun_grammar::TokenKind;
use vun_intern::Symbol;
use vun_sema_int::{Extended, Integer, Interval};

use crate::Value;
use crate::trap::Reason;
use crate::trap::Unevaluated;

/// The most items a compile-time array holds: a repetition past it traps
/// ([`Reason::Size`]) rather than allocate without bound.
pub const MAX_ITEMS: usize = 1 << 20;

impl Value {
    /// `[item; count]`: an array of `count` copies of `item`.
    ///
    /// # Errors
    /// Returns [`Unevaluated::Unsupported`] for a count that is not a non-negative integer, and
    /// a trap for one past [`MAX_ITEMS`].
    pub fn repeat(item: Self, count: &Self) -> Result<Self, Unevaluated> {
        let Self::Int(n) = count else { return Err(Unevaluated::Unsupported) };
        if n.is_negative() {
            return Err(Unevaluated::Unsupported);
        }
        let n = n.to_u128().and_then(|n| usize::try_from(n).ok()).filter(|n| *n <= MAX_ITEMS);
        Ok(Self::Array(vec![item; n.ok_or_else(|| Unevaluated::trap(Reason::Size))?]))
    }

    /// The part `part` of this value.
    ///
    /// # Errors
    /// Returns a trap for an index past the end, and [`Unevaluated::Unsupported`] for a value
    /// without that part.
    pub fn part(&self, part: &Part) -> Result<Self, Unevaluated> {
        match (part, self) {
            // Items at bounds of indices are that run of items, as `xs[a..b]` borrows them.
            (Part::Item(Self::Bounds(bounds)), Self::Array(items)) => {
                Ok(Self::Array(items[bounds.run(items.len())?].to_vec()))
            }
            _ => self.slot(part).cloned(),
        }
    }

    /// The value at `parts` under this one, borrowed unless a run of items is taken.
    ///
    /// # Errors
    /// As [`Self::part`].
    pub fn at(&self, parts: &[Part]) -> Result<Cow<'_, Self>, Unevaluated> {
        let Some((first, rest)) = parts.split_first() else { return Ok(Cow::Borrowed(self)) };
        match (first, self) {
            (Part::Item(Self::Bounds(_)), _) => Ok(Cow::Owned(self.part(first)?.at(rest)?.into_owned())),
            _ => self.slot(first)?.at(rest),
        }
    }

    /// The place in this value that `part` names.
    fn slot(&self, part: &Part) -> Result<&Self, Unevaluated> {
        match (part, self) {
            (Part::Field(name), Self::Struct { fields, .. }) => {
                fields.iter().find(|(n, _)| n == name).map(|(_, v)| v).ok_or(Unevaluated::Unsupported)
            }
            (Part::Item(_), Self::Array(items))
            | (Part::Position(_), Self::Tuple(items))
            | (Part::Payload(_), Self::Variant { payload: items, .. }) => Ok(&items[part.position(items)?]),
            _ => Err(Unevaluated::Unsupported),
        }
    }

    fn slot_mut(&mut self, part: &Part) -> Result<&mut Self, Unevaluated> {
        match (part, self) {
            (Part::Field(name), Self::Struct { fields, .. }) => {
                fields.iter_mut().find(|(n, _)| n == name).map(|(_, v)| v).ok_or(Unevaluated::Unsupported)
            }
            (Part::Item(_), Self::Array(items))
            | (Part::Position(_), Self::Tuple(items))
            | (Part::Payload(_), Self::Variant { payload: items, .. }) => {
                let at = part.position(items)?;
                Ok(&mut items[at])
            }
            _ => Err(Unevaluated::Unsupported),
        }
    }

    /// Replaces the value at `parts` under this one with `value`, in place.
    ///
    /// # Errors
    /// As [`Self::part`]; nothing is changed then.
    pub fn replace(&mut self, parts: &[Part], value: Self) -> Result<(), Unevaluated> {
        match parts.split_first() {
            None => *self = value,
            Some((first, rest)) => self.slot_mut(first)?.replace(rest, value)?,
        }
        Ok(())
    }

    /// This integer brought into the range `range` (spec 04, rule 3): unchanged when it lies
    /// within, and otherwise a trap for [`Narrowing::To`], the value congruent modulo the size of
    /// the range for [`Narrowing::Wrap`], the nearer bound for [`Narrowing::Sat`].
    ///
    /// # Errors
    /// Returns a trap when `.to` fails or the range is unbounded for `.wrap`,
    /// and [`Unevaluated::Unsupported`] for a value that is no integer.
    pub fn narrow(&self, range: &Interval, narrowing: Narrowing) -> Result<Self, Unevaluated> {
        let Self::Int(n) = self else { return Err(Unevaluated::Unsupported) };
        if range.contains(&Interval::singleton(n.clone())) {
            return Ok(self.clone());
        }
        let bound = |b: &Extended| b.finite().cloned().ok_or_else(|| Unevaluated::trap(Reason::Narrowing));
        let n = match narrowing {
            Narrowing::To => return Err(Unevaluated::trap(Reason::Narrowing)),
            Narrowing::Wrap => {
                let (lo, hi) = (bound(range.lo())?, bound(range.hi())?);
                let size = &(&hi - &lo) + &Integer::from(1);
                &lo + &(n - &lo).modulo(&size).ok_or_else(|| Unevaluated::trap(Reason::Narrowing))?
            }
            Narrowing::Sat if Extended::Finite(n.clone()) < *range.lo() => bound(range.lo())?,
            Narrowing::Sat => bound(range.hi())?,
        };
        Ok(Self::Int(n))
    }

    /// `lo..hi`, `lo..=hi` or `lo..`, by `op`.
    ///
    /// # Errors
    /// Returns [`Unevaluated::Unsupported`] for bounds that are not integers.
    pub fn range(op: TokenKind, lo: &Self, hi: Option<&Self>) -> Result<Self, Unevaluated> {
        let Self::Int(start) = lo else { return Err(Unevaluated::Unsupported) };
        let end = match (hi, op) {
            (None, _) => None,
            (Some(Self::Int(hi)), TokenKind::DotDotEq) => Some(hi + &Integer::from(1)),
            (Some(Self::Int(hi)), _) => Some(hi.clone()),
            (Some(_), _) => return Err(Unevaluated::Unsupported),
        };
        Ok(Self::Bounds(Endpoints { start: start.clone(), end }))
    }

    /// This value where a `lang.Range` may be expected: bounds are the set of their items (spec
    /// 04, section 1), any other value itself.
    #[must_use]
    pub fn ranged(self) -> Self {
        match self {
            Self::Bounds(bounds) => Self::Range(bounds.items()),
            value => value,
        }
    }
}

/// Bounds at compile time: from `start` up to `end`, which they stop before, or without an end.
/// `a..=b` is `a..b + 1`. They keep where they are even holding nothing: `xs[10..10]` is past the
/// end of three items, `xs[3..3]` is not.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Endpoints {
    pub start: Integer,
    pub end: Option<Integer>,
}

impl Endpoints {
    /// The integers these bounds hold, as the `lang.Range` they are accepted as.
    #[must_use]
    pub fn items(&self) -> Option<Interval> {
        match &self.end {
            Some(end) => Interval::half_open(self.start.clone(), end),
            None => Some(Interval::at_least(self.start.clone())),
        }
    }

    /// The indices these bounds cover among `len` items: an open end reaches the last; bounds
    /// starting after they end or reaching past the last item trap.
    fn run(&self, len: usize) -> Result<std::ops::Range<usize>, Unevaluated> {
        let index = |n: &Integer| n.to_i128().and_then(|n| usize::try_from(n).ok());
        let end = self.end.as_ref().map_or(Some(len), index);
        match (index(&self.start), end) {
            (Some(start), Some(end)) if start <= end && end <= len => Ok(start..end),
            _ => Err(Unevaluated::trap(Reason::Bounds)),
        }
    }
}

/// A part of a compile-time value: a struct's field, an array's item at an integer index, a
/// tuple's item at a position, or an item of a variant's payload.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Part {
    Field(Symbol),
    Item(Value),
    Position(usize),
    Payload(usize),
}

impl Part {
    /// Where this part is among `parts`: an index past the end traps.
    fn position(&self, parts: &[Value]) -> Result<usize, Unevaluated> {
        let at = match self {
            Self::Item(Value::Int(i)) => i.to_i128().and_then(|i| usize::try_from(i).ok()),
            Self::Position(at) | Self::Payload(at) => Some(*at),
            Self::Item(_) | Self::Field(_) => return Err(Unevaluated::Unsupported),
        };
        at.filter(|&at| at < parts.len()).ok_or_else(|| Unevaluated::trap(Reason::Bounds))
    }
}
