//! What operators compute (spec 04): on compile-time values, where integer arithmetic is exact and
//! a division by zero is a trap, and on integer types, by interval arithmetic; and the parts of
//! compile-time values. Sema and the interpreter both compute through here.

use overt_grammar::{Narrowing, TokenKind};
use overt_intern::Symbol;

use crate::env::Unevaluated;
use crate::int::{Extended, Integer, Interval};
use crate::trap::Reason;
use crate::ty::Value;

impl Value {
    /// `left op right`.
    ///
    /// # Errors
    /// Returns a trap for a division by zero or a shift past any width, and
    /// [`Unevaluated::Unsupported`] for operands or an operator not computed yet.
    pub fn binary(op: TokenKind, left: &Self, right: &Self) -> Result<Self, Unevaluated> {
        if let (TokenKind::PlusPlus, Self::Array(a), Self::Array(b)) = (op, left, right) {
            return Ok(Self::Array(a.iter().chain(b).cloned().collect()));
        }
        let (Self::Int(a), Self::Int(b)) = (left, right) else { return Err(Unevaluated::Unsupported) };
        let n = match op {
            TokenKind::Plus => a + b,
            TokenKind::Minus => a - b,
            TokenKind::Star => a * b,
            TokenKind::Slash => a.checked_div(b).ok_or_else(|| Unevaluated::trap(Reason::DivisionByZero))?,
            TokenKind::Percent => {
                a.checked_rem(b).ok_or_else(|| Unevaluated::trap(Reason::DivisionByZero))?
            }
            TokenKind::Shl => a.shifted(b).ok_or_else(|| Unevaluated::trap(Reason::Shift))?,
            TokenKind::Shr => a.shifted(&-b).ok_or_else(|| Unevaluated::trap(Reason::Shift))?,
            _ => return Err(Unevaluated::Unsupported),
        };
        Ok(Self::Int(n))
    }

    /// `[item; count]`: an array of `count` copies of `item`.
    ///
    /// # Errors
    /// Returns [`Unevaluated::Unsupported`] for a count that is not an integer that fits a
    /// `usize`.
    pub fn repeat(item: Self, count: &Self) -> Result<Self, Unevaluated> {
        let Self::Int(n) = count else { return Err(Unevaluated::Unsupported) };
        let n = n.to_i128().and_then(|n| usize::try_from(n).ok()).ok_or(Unevaluated::Unsupported)?;
        Ok(Self::Array(vec![item; n]))
    }

    /// The part `part` of this value.
    ///
    /// # Errors
    /// Returns a trap for an index past the end, and [`Unevaluated::Unsupported`] for a value
    /// without that part.
    pub fn part(&self, part: &Part) -> Result<Self, Unevaluated> {
        let parts = match (part, self) {
            (Part::Field(name), Self::Struct { fields, .. }) => {
                return fields
                    .iter()
                    .find(|(n, _)| n == name)
                    .map(|(_, v)| v.clone())
                    .ok_or(Unevaluated::Unsupported);
            }
            (Part::Item(_), Self::Array(items)) | (Part::Position(_), Self::Tuple(items)) => items,
            (Part::Payload(_), Self::Variant { payload, .. }) => payload,
            _ => return Err(Unevaluated::Unsupported),
        };
        Ok(parts[part.position(parts)?].clone())
    }

    /// This value with its part `part` replaced by `value`.
    ///
    /// # Errors
    /// As [`Self::part`].
    pub fn with_part(mut self, part: &Part, value: Self) -> Result<Self, Unevaluated> {
        let slot = match (part, &mut self) {
            (Part::Field(name), Self::Struct { fields, .. }) => {
                &mut fields.iter_mut().find(|(n, _)| n == name).ok_or(Unevaluated::Unsupported)?.1
            }
            (Part::Item(_), Self::Array(items))
            | (Part::Position(_), Self::Tuple(items))
            | (Part::Payload(_), Self::Variant { payload: items, .. }) => {
                let at = part.position(items)?;
                &mut items[at]
            }
            _ => return Err(Unevaluated::Unsupported),
        };
        *slot = value;
        Ok(self)
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

    /// Whether the comparison `left op right` of two integers holds; `None` for another operator
    /// or other operands.
    #[must_use]
    pub fn compare(op: TokenKind, left: &Self, right: &Self) -> Option<bool> {
        let (Self::Int(a), Self::Int(b)) = (left, right) else { return None };
        Some(match op {
            TokenKind::EqEq => a == b,
            TokenKind::BangEq => a != b,
            TokenKind::Lt => a < b,
            TokenKind::Le => a <= b,
            TokenKind::Gt => a > b,
            TokenKind::Ge => a >= b,
            _ => return None,
        })
    }

    /// `op operand`.
    ///
    /// # Errors
    /// Returns [`Unevaluated::Unsupported`] for an operand or an operator not computed yet.
    pub fn unary(op: TokenKind, operand: &Self) -> Result<Self, Unevaluated> {
        match (op, operand) {
            (TokenKind::Minus, Self::Int(n)) => Ok(Self::Int(-n)),
            _ => Err(Unevaluated::Unsupported),
        }
    }

    /// `lo..hi`, `lo..=hi` or `lo..`, by `op`.
    ///
    /// # Errors
    /// Returns [`Unevaluated::Unsupported`] for bounds that are not integers.
    pub fn range(op: TokenKind, lo: &Self, hi: Option<&Self>) -> Result<Self, Unevaluated> {
        let Self::Int(lo) = lo else { return Err(Unevaluated::Unsupported) };
        let range = match (hi, op) {
            (None, _) => Some(Interval::at_least(lo.clone())),
            (Some(Self::Int(hi)), TokenKind::DotDotEq) => Interval::closed(lo.clone(), hi.clone()),
            (Some(Self::Int(hi)), _) => Interval::half_open(lo.clone(), hi),
            (Some(_), _) => return Err(Unevaluated::Unsupported),
        };
        Ok(Self::Range(range))
    }
}

impl Interval {
    /// Every item of some `a..b`, `a..=b` or `a..` (by `op`) whose bounds lie in `lo` and `hi`:
    /// the type of the items of `lang.Bounds`. `None` when no such range holds an item.
    #[must_use]
    pub fn items(op: TokenKind, lo: &Self, hi: Option<&Self>) -> Option<Self> {
        let top = match (hi.map(Self::hi), op) {
            (None | Some(Extended::PosInf), _) => Extended::PosInf,
            (Some(Extended::Finite(hi)), TokenKind::DotDot) => Extended::Finite(hi - &Integer::from(1)),
            (Some(hi), _) => hi.clone(),
        };
        Self::new(lo.lo().clone(), top)
    }

    /// The operators whose integer impls exist on `uN` types only and keep the type (rule 8).
    pub const BITWISE: [TokenKind; 5] =
        [TokenKind::Amp, TokenKind::Pipe, TokenKind::Caret, TokenKind::Shl, TokenKind::Shr];

    /// Whether this is the range of a `uN` type, `0..=2^N - 1`, which bit operations need.
    #[must_use]
    pub fn is_unsigned_width(&self) -> bool {
        let (Some(bits), Some(count)) = (self.bits(), self.count()) else { return false };
        !self.is_signed() && Integer::from(1).shifted(&Integer::from(u128::from(bits))) == Some(count)
    }

    /// The range of `self op other` over integers of these ranges (rule 1); `None` for an
    /// operator not typed yet, or a division by a divisor that is always zero.
    #[must_use]
    pub fn binary(&self, op: TokenKind, other: &Self) -> Option<Self> {
        match op {
            TokenKind::Plus => Some(self.add(other)),
            TokenKind::Minus => Some(self.sub(other)),
            TokenKind::Star => Some(self.mul(other)),
            TokenKind::Slash => self.div(other).range,
            TokenKind::Percent => self.rem(other).range,
            _ => None,
        }
    }
}

/// A part of a compile-time value: a struct's field, an array's item at an integer index, a
/// tuple's item at a position, or an item of a variant's payload.
#[derive(Clone, Debug)]
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
