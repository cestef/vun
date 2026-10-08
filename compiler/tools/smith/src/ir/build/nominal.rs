//! Forms of options, results, structs and enums, and the matching of them.

use super::Untyped;
use crate::int::Range;
use crate::ir::{Arm, Expr, Kind, Pattern};
use crate::ty::{Nominal, Ty};

impl Expr {
    /// `operand?` in a function that gives `result`: an option in one giving an option, a result
    /// in one giving a result of the same error type.
    ///
    /// # Errors
    /// Returns why the operand is not one the function can propagate.
    pub fn propagate(operand: Self, result: &Ty) -> Result<Self, Untyped> {
        let (Ty::Enum(have, held), Ty::Enum(returns, given)) = (&operand.ty, result) else {
            return Err(Untyped::Operand);
        };
        let (payload, error) = match have {
            Nominal::Option => (1, None),
            Nominal::Result => (0, Some(1)),
            Nominal::Declared(_) => return Err(Untyped::Operand),
        };
        let same_error = error.is_none_or(|at| held[at] == given[at]);
        (have == returns && same_error).then_some(()).ok_or(Untyped::Mismatch)?;
        let ty = held[payload][0].clone();
        Ok(Self { ty, kind: Kind::Try(Box::new(operand)) })
    }

    /// A value of the struct `ty`, of fields its types accept.
    ///
    /// # Errors
    /// Returns why the fields do not fit the struct.
    pub fn structure(ty: &Ty, fields: Vec<Self>) -> Result<Self, Untyped> {
        let Ty::Struct(id, want) = ty else { return Err(Untyped::Operand) };
        Self::accepts(want, &fields)?;
        Ok(Self { ty: ty.clone(), kind: Kind::Struct(*id, fields) })
    }

    /// A value of the enum `ty`: its `variant`, holding `payload`.
    ///
    /// # Errors
    /// Returns why the payload does not fit the variant.
    pub fn variant(ty: &Ty, variant: usize, payload: Vec<Self>) -> Result<Self, Untyped> {
        let Ty::Enum(nominal, variants) = ty else { return Err(Untyped::Operand) };
        Self::accepts(variants.get(variant).ok_or(Untyped::Bounds)?, &payload)?;
        Ok(Self { ty: ty.clone(), kind: Kind::Variant(*nominal, variant, payload) })
    }

    pub(super) fn accepts(want: &[Ty], items: &[Self]) -> Result<(), Untyped> {
        let fits = want.len() == items.len() && want.iter().zip(items).all(|(w, i)| w.accepts(&i.ty));
        fits.then_some(()).ok_or(Untyped::Mismatch)
    }

    /// `operand.try_to(to)`: the option of `to` that holds the operand where it fits.
    ///
    /// # Errors
    /// Returns why the operand is not an integer.
    pub fn try_to(operand: Self, to: Range) -> Result<Self, Untyped> {
        operand.range()?;
        let ty = Ty::Enum(Nominal::Option, vec![Vec::new(), vec![Ty::Int(to)]]);
        Ok(Self { ty, kind: Kind::TryTo(Box::new(operand)) })
    }

    /// `option.unwrap_or(default)`: what the option holds, or the default.
    ///
    /// # Errors
    /// Returns why the operands are not an option and a value it holds.
    pub fn unwrap_or(option: Self, default: Self) -> Result<Self, Untyped> {
        let Ty::Enum(Nominal::Option, variants) = &option.ty else { return Err(Untyped::Operand) };
        let held = variants[1][0].clone();
        held.accepts(&default.ty).then_some(()).ok_or(Untyped::Mismatch)?;
        Ok(Self { ty: held, kind: Kind::UnwrapOr(Box::new(option), Box::new(default)) })
    }

    /// `option.is_some()`
    ///
    /// # Errors
    /// Returns why the operand is not an option.
    pub fn is_some(option: Self) -> Result<Self, Untyped> {
        matches!(option.ty, Ty::Enum(Nominal::Option, _)).then_some(()).ok_or(Untyped::Operand)?;
        Ok(Self { ty: Ty::Bool, kind: Kind::IsSome(Box::new(option)) })
    }

    /// The arm of `scrutinee`, an enum, that its value is: every variant has an arm or `_` does,
    /// and the arms' values have a type in common.
    ///
    /// # Errors
    /// Returns why the arms do not cover the variants or have no type in common.
    pub fn matching(scrutinee: Self, arms: Vec<Arm>) -> Result<Self, Untyped> {
        let Ty::Enum(_, variants) = &scrutinee.ty else { return Err(Untyped::Operand) };
        let mut covered = vec![false; variants.len()];
        let mut open = true;
        for (at, arm) in arms.iter().enumerate() {
            match &arm.pattern {
                Pattern::Variant { variant, binds } => {
                    let payload = variants.get(*variant).ok_or(Untyped::Bounds)?;
                    (binds.len() == payload.len() && !std::mem::replace(&mut covered[*variant], true))
                        .then_some(())
                        .ok_or(Untyped::Mismatch)?;
                }
                Pattern::Any => {
                    (at + 1 == arms.len()).then_some(()).ok_or(Untyped::Mismatch)?;
                    open = false;
                }
            }
        }
        let all = covered.iter().all(|c| *c);
        (open == all).then_some(()).ok_or(Untyped::Mismatch)?;
        let ty = arms
            .iter()
            .try_fold(None::<Ty>, |held, arm| match held {
                None => Some(Some(arm.body.ty.clone())),
                Some(t) => t.join(&arm.body.ty).map(Some),
            })
            .flatten()
            .ok_or(Untyped::Mismatch)?;
        ty.fits_a_word().then_some(()).ok_or(Untyped::Width)?;
        Ok(Self { ty, kind: Kind::Match(Box::new(scrutinee), arms) })
    }
}
