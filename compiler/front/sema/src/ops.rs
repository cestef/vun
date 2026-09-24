//! What operators compute (spec 04): on compile-time values, where integer arithmetic is exact and
//! a division by zero is a trap, and on integer types, by interval arithmetic. Sema and the
//! interpreter both compute through here.

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

    /// `self[index]`: the item of an array value at an integer index.
    ///
    /// # Errors
    /// Returns a trap for an index past the end, and [`Unevaluated::Unsupported`] for another
    /// value or index.
    pub fn item(&self, index: &Self) -> Result<Self, Unevaluated> {
        let Self::Array(items) = self else { return Err(Unevaluated::Unsupported) };
        items.get(Self::position(items, index)?).cloned().ok_or(Unevaluated::Unsupported)
    }

    /// This array with the item at `index` replaced by `item`.
    ///
    /// # Errors
    /// As [`Self::item`].
    pub fn with_item(self, index: &Self, item: Self) -> Result<Self, Unevaluated> {
        let Self::Array(mut items) = self else { return Err(Unevaluated::Unsupported) };
        let at = Self::position(&items, index)?;
        items[at] = item;
        Ok(Self::Array(items))
    }

    /// This struct value with its field `name` replaced by `value`.
    ///
    /// # Errors
    /// Returns [`Unevaluated::Unsupported`] for a value without that field.
    pub fn with_field(self, name: Symbol, value: Self) -> Result<Self, Unevaluated> {
        let Self::Struct { ty, mut fields } = self else { return Err(Unevaluated::Unsupported) };
        let slot = fields.iter_mut().find(|(n, _)| *n == name).ok_or(Unevaluated::Unsupported)?;
        slot.1 = value;
        Ok(Self::Struct { ty, fields })
    }

    /// Where the integer `index` is among `items`: a trap past the end.
    fn position(items: &[Self], index: &Self) -> Result<usize, Unevaluated> {
        let Self::Int(i) = index else { return Err(Unevaluated::Unsupported) };
        let at = i.to_i128().and_then(|i| usize::try_from(i).ok());
        at.filter(|&at| at < items.len()).ok_or_else(|| Unevaluated::trap(Reason::Bounds))
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
