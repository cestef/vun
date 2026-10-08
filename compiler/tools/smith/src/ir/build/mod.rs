//! Making forms: each checks what the spec asks of its operands and gives the type the spec
//! says, so that an expression that exists is well typed.

use std::fmt;

use super::{Arith, Compare, Convert, Expr, Kind, LocalId, Logic, Narrow, Place, Stmt, Unary};
use crate::float::{Float, Format};
use crate::int::{BigInt, Range};
use crate::ty::Ty;
use crate::value::Value;

/// Why no form can be made of the operands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Untyped {
    /// An operand is not of the kind the form is for.
    Operand,
    /// The result needs more bits than a run-time integer may (spec 04, rule 5).
    Width,
    /// The divisor may be zero (rule 6).
    Zero,
    /// A bit operation outside the `uN` types, or a shift by more than `N - 1` (rule 8).
    Bits,
    /// The branches, or the items, have no type in common.
    Mismatch,
    /// An index that may be past the end, or a part that does not exist.
    Bounds,
}

impl fmt::Display for Untyped {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Operand => "an operand is not of the kind the form is for",
            Self::Width => "the result is wider than a run-time integer",
            Self::Zero => "the divisor may be zero",
            Self::Bits => "bit operations are on the unsigned types, shifts by less than their width",
            Self::Mismatch => "the parts have no type in common",
            Self::Bounds => "the index may be past the end",
        })
    }
}

impl std::error::Error for Untyped {}

mod calls;
mod nominal;

impl Expr {
    #[must_use]
    pub fn int(n: impl Into<BigInt>) -> Self {
        let n = n.into();
        Self { ty: Ty::Int(Range::exactly(n.clone())), kind: Kind::Lit(Value::Int(n)) }
    }

    #[must_use]
    pub fn float(format: Format, value: f64) -> Self {
        let rounded = format.round(value);
        let finite = if rounded.is_finite() { rounded } else { format.greatest().copysign(rounded) };
        let value = Float::of(finite);
        Self { ty: Ty::Float(format), kind: Kind::Lit(Value::Float(value)) }
    }

    /// An integer or a float as the float type `to`, or a float as an integer type.
    ///
    /// # Errors
    /// Returns why the operand has no such conversion.
    pub fn convert(how: Convert, operand: Self, to: Ty) -> Result<Self, Untyped> {
        let from_float = matches!(operand.ty, Ty::Float(_));
        let fits = match (how, &to) {
            (Convert::Nearest, Ty::Float(_)) => from_float || operand.ty.range().is_some(),
            (Convert::Truncate, Ty::Int(_)) => from_float,
            _ => false,
        };
        fits.then(|| Self { ty: to, kind: Kind::Convert(how, Box::new(operand)) }).ok_or(Untyped::Operand)
    }

    #[must_use]
    pub fn bool(b: bool) -> Self {
        Self { ty: Ty::Bool, kind: Kind::Lit(Value::Bool(b)) }
    }

    #[must_use]
    pub fn unit() -> Self {
        Self { ty: Ty::Unit, kind: Kind::Lit(Value::Unit) }
    }

    #[must_use]
    pub fn local(id: LocalId, ty: &Ty) -> Self {
        Self { ty: ty.clone(), kind: Kind::Local(id) }
    }

    fn range(&self) -> Result<&Range, Untyped> {
        self.ty.range().ok_or(Untyped::Operand)
    }

    /// # Errors
    /// Returns why the operand has no such operator.
    pub fn unary(op: Unary, operand: Self) -> Result<Self, Untyped> {
        let ty = match op {
            Unary::Neg if matches!(operand.ty, Ty::Float(_)) => operand.ty.clone(),
            Unary::Neg => Ty::Int(operand.range()?.negation()),
            Unary::BitNot => {
                operand.range()?.word().ok_or(Untyped::Bits)?;
                operand.ty.clone()
            }
            Unary::Not => (operand.ty == Ty::Bool).then_some(Ty::Bool).ok_or(Untyped::Operand)?,
        };
        Self::wide(Self { ty, kind: Kind::Unary(op, Box::new(operand)) })
    }

    /// # Errors
    /// Returns why the operands have no such operation.
    pub fn arith(op: Arith, left: Self, right: Self) -> Result<Self, Untyped> {
        if let (Ty::Float(a), Ty::Float(b)) = (&left.ty, &right.ty) {
            let float = matches!(op, Arith::Add | Arith::Sub | Arith::Mul | Arith::Div | Arith::Rem);
            return (a == b && float)
                .then(|| Self { ty: left.ty.clone(), kind: Kind::Arith(op, Box::new(left), Box::new(right)) })
                .ok_or(Untyped::Operand);
        }
        let (a, b) = (left.range()?, right.range()?);
        let range = match op {
            Arith::Add => a.sum(b),
            Arith::Sub => a.difference(b),
            Arith::Mul => a.product(b),
            Arith::Div => a.quotient(b).filter(|_| !b.contains(&BigInt::ZERO)).ok_or(Untyped::Zero)?,
            Arith::Rem => a.remainder(b).filter(|_| !b.contains(&BigInt::ZERO)).ok_or(Untyped::Zero)?,
            Arith::And | Arith::Or | Arith::Xor => a.bitwise(b).ok_or(Untyped::Bits)?,
            Arith::Shl | Arith::Shr => a.shifted(b).ok_or(Untyped::Bits)?,
        };
        if matches!(op, Arith::Div | Arith::Rem) {
            a.hull(b).hull(&range).fits_a_word().then_some(()).ok_or(Untyped::Width)?;
        }
        Self::wide(Self { ty: Ty::Int(range), kind: Kind::Arith(op, Box::new(left), Box::new(right)) })
    }

    /// # Errors
    /// Returns why the operands cannot be compared.
    pub fn compare(op: Compare, left: Self, right: Self) -> Result<Self, Untyped> {
        if matches!((&left.ty, &right.ty), (Ty::Float(a), Ty::Float(b)) if a == b) {
            return Ok(Self { ty: Ty::Bool, kind: Kind::Compare(op, Box::new(left), Box::new(right)) });
        }
        left.range()?.hull(right.range()?).fits_a_word().then_some(()).ok_or(Untyped::Width)?;
        Ok(Self { ty: Ty::Bool, kind: Kind::Compare(op, Box::new(left), Box::new(right)) })
    }

    /// # Errors
    /// Returns why the operands are not bools.
    pub fn logic(op: Logic, left: Self, right: Self) -> Result<Self, Untyped> {
        (left.ty == Ty::Bool && right.ty == Ty::Bool).then_some(()).ok_or(Untyped::Operand)?;
        Ok(Self { ty: Ty::Bool, kind: Kind::Logic(op, Box::new(left), Box::new(right)) })
    }

    /// A borrow of `place`, of type `ty`, which `mutable` says is `&mut`.
    #[must_use]
    pub fn borrow(mutable: bool, place: Place, ty: Ty) -> Self {
        Self { ty: Ty::Ref(mutable, Box::new(ty)), kind: Kind::Borrow(mutable, place) }
    }

    /// What the reference `operand` refers to.
    ///
    /// # Errors
    /// Returns why the operand is no reference.
    pub fn deref(operand: Self) -> Result<Self, Untyped> {
        let Ty::Ref(_, inner) = &operand.ty else { return Err(Untyped::Operand) };
        Ok(Self { ty: (**inner).clone(), kind: Kind::Deref(Box::new(operand)) })
    }

    /// `operand` as the integer type `to`: `To` only where `to` holds every value it can have.
    ///
    /// # Errors
    /// Returns why the operand is not an integer, or may trap.
    pub fn narrow(how: Narrow, operand: Self, to: Range) -> Result<Self, Untyped> {
        let from = operand.range()?;
        if how == Narrow::To && !from.within(&to) {
            return Err(Untyped::Bounds);
        }
        Ok(Self { ty: Ty::Int(to), kind: Kind::Narrow(how, Box::new(operand)) })
    }

    /// # Errors
    /// Returns why the condition is not a bool or the branches have no type in common.
    pub fn when(condition: Self, then: Self, otherwise: Self) -> Result<Self, Untyped> {
        (condition.ty == Ty::Bool).then_some(()).ok_or(Untyped::Operand)?;
        let ty = then.ty.join(&otherwise.ty).ok_or(Untyped::Mismatch)?;
        ty.fits_a_word().then_some(()).ok_or(Untyped::Width)?;
        Ok(Self { ty, kind: Kind::If(Box::new(condition), Box::new(then), Box::new(otherwise)) })
    }

    #[must_use]
    pub fn tuple(items: Vec<Self>) -> Self {
        let ty = Ty::Tuple(items.iter().map(|i| i.ty.clone()).collect());
        Self { ty, kind: Kind::Tuple(items) }
    }

    /// # Errors
    /// Returns why the items have no type in common.
    pub fn array(items: Vec<Self>) -> Result<Self, Untyped> {
        let item = items
            .iter()
            .try_fold(None::<Ty>, |held, i| match held {
                None => Some(Some(i.ty.clone())),
                Some(t) => t.join(&i.ty).map(Some),
            })
            .flatten()
            .ok_or(Untyped::Mismatch)?;
        item.fits_a_word().then_some(()).ok_or(Untyped::Width)?;
        Ok(Self { ty: Ty::Array(items.len(), Box::new(item)), kind: Kind::Array(items) })
    }

    #[must_use]
    pub fn repeat(item: Self, count: usize) -> Self {
        Self { ty: Ty::Array(count, Box::new(item.ty.clone())), kind: Kind::Repeat(Box::new(item), count) }
    }

    /// # Errors
    /// Returns why the operand has no such part.
    pub fn field(tuple: Self, at: usize) -> Result<Self, Untyped> {
        let (Ty::Tuple(parts) | Ty::Struct(_, parts)) = &tuple.ty else { return Err(Untyped::Operand) };
        let ty = parts.get(at).ok_or(Untyped::Bounds)?.clone();
        Ok(Self { ty, kind: Kind::Field(Box::new(tuple), at) })
    }

    /// # Errors
    /// Returns why the operand is not an array or the index may be past its end.
    pub fn index(array: Self, index: Self) -> Result<Self, Untyped> {
        let Ty::Array(len, item) = &array.ty else { return Err(Untyped::Operand) };
        let last = BigInt::from(*len) - BigInt::from(1);
        index.range()?.within(&Range::new(0, last)).then_some(()).ok_or(Untyped::Bounds)?;
        let ty = (**item).clone();
        Ok(Self { ty, kind: Kind::Index(Box::new(array), Box::new(index)) })
    }

    #[must_use]
    pub fn block(stmts: Vec<Stmt>, tail: Self) -> Self {
        Self { ty: tail.ty.clone(), kind: Kind::Block(stmts, Box::new(tail)) }
    }

    /// `self` if its type is within the widest an integer may be.
    fn wide(self) -> Result<Self, Untyped> {
        if self.ty.fits_a_word() { Ok(self) } else { Err(Untyped::Width) }
    }
}
