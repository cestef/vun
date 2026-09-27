//! Fitting a line's operands to an arm64 form: the instruction's width is the one its mnemonic's
//! spelling gives (`ldrb`), else its registers at positions of the instruction's width (`w0`,
//! `x0`), else the form's only one; each position takes what the encoder's `Kind::encode` does,
//! over the operand the backends make of it, a register of the position's class and width, and an
//! immediate the operation's bits hold.

use vun_inline::{FormId, Line, OpId, SizeId};
use vun_isa::imm::held;
use vun_isa::{Blocks, Fit, Fixed, Given, Label, Misfit, Place, Via, Why, positions};

use crate::{Arm64, Barrier, Class, Kind, Op, Operand, Reg, Size, Width};

impl Size {
    /// Its index in the table, as a block names it.
    #[must_use]
    pub const fn id(self) -> SizeId {
        SizeId(self as u8)
    }
}

impl Kind {
    /// Whether a register at this position has the instruction's width.
    const fn sized(self) -> bool {
        matches!(self, Self::Gpr(_, Width::Inst) | Self::Fpr(Width::Inst) | Self::Shifted(_))
    }

    /// The class of register this position takes, if it takes one, and the width it reads it at
    /// in an instruction of `size`, if a fixed one.
    const fn register(self, size: Size) -> Option<(Class, Option<Size>)> {
        match self {
            Self::Gpr(_, width) => Some((Class::General, Some(width.at(size)))),
            Self::Shifted(_) => Some((Class::General, Some(size))),
            Self::Extended => Some((Class::General, None)),
            Self::Fpr(width) => Some((Class::Fp, Some(width.at(size)))),
            _ => None,
        }
    }

    /// Whether this position of an instruction of `size` takes `given`.
    fn fits<V: Clone, P: Clone, S: Copy>(self, size: Size, given: &Given<V, P, S>) -> Result<(), Why> {
        if let (Some(class), Some((want, width))) = (given.class(), self.register(size)) {
            if Class::ALL[usize::from(class.0)] != want {
                return Err(Why::Class);
            }
            if let (Some(given), Some(width)) = (given.size(), width)
                && Size::of(given) != width
            {
                return Err(Why::Kind);
            }
        }
        match (self, given) {
            (_, Given::Imm(None)) => {
                return Err(if matches!(self, Self::Imm(_)) { Why::Wide } else { Why::Kind });
            }
            (Self::Barrier, Given::Choice(k)) if usize::from(*k) < Barrier::ALL.len() => {}
            (Self::Mem(..), Given::Mem(mem)) => Arm64::addressable(mem, self).map_err(|_| Why::Kind)?,
            (_, Given::Choice(_) | Given::Mem(_)) => return Err(Why::Kind),
            _ => {}
        }
        let operand = Arm64::operand(&given.clone().operand(), self, &mut Representative);
        match (self.encode(&operand, size), given) {
            (None, Given::Imm(_)) if matches!(self, Self::Imm(_)) => Err(Why::Wide),
            (None, _) => Err(Why::Kind),
            (Some(_), Given::Imm(Some(v))) if !held(*v, size.bits()) => Err(Why::Wide),
            (Some(_), _) => Ok(()),
        }
    }
}

/// The operands of a line as the encoder sees them before registers are given: a binding's
/// register stands for any general one.
struct Representative;

impl<S: Copy> Place<Arm64, S> for Representative {
    fn bound(&mut self) -> Reg {
        Reg::X0
    }

    fn line(&self, _: Line) -> Operand<S> {
        Operand::Label(Label::new(0))
    }

    fn via(&self, _: S) -> Via {
        Via::Direct
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
    let kinds = form.args.iter().map(|arg| arg.kind);
    let positions = positions(kinds, fixed, |k| matches!(k, Kind::Cond(_)), givens)?;
    let mut known = fixed.size;
    for p in positions.iter().filter(|p| p.kind.sized()) {
        let Some(width) = p.given.size() else { continue };
        if known.is_some_and(|k| k != width) {
            return Err(p.misfit(Why::Kind));
        }
        known = Some(width);
    }
    let size = match known {
        Some(width) => Size::of(width),
        None if form.sizes.len() == 1 || !positions.iter().any(|p| p.kind.sized()) => {
            form.sizes.first().ok_or(Misfit::Width)?.size
        }
        None => return Err(Misfit::Unsized),
    };
    if form.at(size).is_none() {
        return Err(Misfit::Width);
    }
    for p in &positions {
        p.kind.fits(size, &p.given).map_err(|why| p.misfit(why))?;
    }
    Ok(Fit { size: size.id(), operands: positions.into_iter().map(|p| p.given.operand()).collect() })
}
