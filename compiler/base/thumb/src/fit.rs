//! Fitting a line's operands to a Thumb form: every form is 32 bits wide; a register position takes
//! a register of its class, a choice position a choice of its kind within its enum.

use vun_inline::{FormId, OpId};
use vun_isa::{ChoiceId, Fit, Fixed, Given, Misfit, Why, positions};

use crate::{Barrier, Choice, Class, Kind, Mask, Op, Size};

impl Kind {
    /// The class of register this position takes, if it takes one.
    const fn class(self) -> Option<Class> {
        match self {
            Self::R => Some(Class::General),
            Self::Special => Some(Class::Special),
            Self::Mask | Self::Barrier => None,
        }
    }

    /// The kind of choice this position takes, if it takes one, and how many there are.
    const fn choice(self) -> Option<(Choice, usize)> {
        match self {
            Self::Mask => Some((Choice::Mask, Mask::ALL.len())),
            Self::Barrier => Some((Choice::Barrier, Barrier::ALL.len())),
            Self::R | Self::Special => None,
        }
    }

    /// Whether this position takes `given`.
    fn fits<V, P, S>(self, given: &Given<V, P, S>) -> Result<(), Why> {
        match (self.class(), self.choice(), given) {
            (Some(want), _, Given::Reg { reg, .. }) if Class::ALL[usize::from(reg.class.0)] != want => {
                Err(Why::Class)
            }
            (Some(Class::General), _, Given::Bound { class, .. })
                if Class::ALL[usize::from(class.0)] != Class::General =>
            {
                Err(Why::Class)
            }
            (Some(_), _, Given::Reg { .. }) | (Some(Class::General), _, Given::Bound { .. }) => Ok(()),
            (_, Some((choice, count)), Given::Choice { of, index })
                if *of == ChoiceId(choice as u8) && usize::from(*index) < count =>
            {
                Ok(())
            }
            _ => Err(Why::Kind),
        }
    }
}

/// The width `givens` give the form `form` of `op`, and each as the form takes it.
pub(crate) fn fit<V: Clone, P: Clone, S: Copy>(
    op: OpId,
    form: FormId,
    fixed: Fixed,
    givens: &[Given<V, P, S>],
) -> Result<Fit<V, P, S>, Misfit> {
    let form = Op::entry(op, form).1;
    let positions = positions(form.kinds(), fixed, |_| false, givens)?;
    let size = match fixed.size {
        Some(size) => Size::of(size),
        None => *form.sizes.first().ok_or(Misfit::Width)?,
    };
    if !form.sizes.contains(&size) {
        return Err(Misfit::Width);
    }
    for p in &positions {
        p.kind.fits(&p.given).map_err(|why| p.misfit(why))?;
    }
    Ok(Fit { size: size.id(), operands: positions.into_iter().map(|p| p.given.operand()).collect() })
}
