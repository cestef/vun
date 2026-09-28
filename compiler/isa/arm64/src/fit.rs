//! Fitting a line's operands to an arm64 form: the instruction's width is the one its mnemonic's
//! spelling gives (`ldrb`), else its registers at positions of the instruction's width (`w0`,
//! `x0`), else the form's only one; each position takes what the encoder's `Kind::encode` does,
//! over the operand the backends make of it, a register of the position's class and width, and an
//! immediate the operation's bits hold.

use vun_inline::{ClassId, FormId, Line, OpId, RegRef, SizeId};
use vun_isa::imm::held;
use vun_isa::word::Field;
use vun_isa::{
    Blocks, Fit, Fixed, Given, Label, Misfit, Place, Takes, Via, Why, Writeback, positions, width,
};

use crate::{Arm64, Barrier, Choice, Class, Kind, Mode, Op, Operand, Size, SysReg, Width};

impl Class {
    /// Its index in the table, as a block names it.
    #[must_use]
    pub const fn id(self) -> ClassId {
        ClassId(self as u8)
    }
}

impl Size {
    /// The width a general register holding a value of this width is named at: 32 or 64 bits,
    /// so `ldrb w0` reads a byte into `w0`.
    const fn named(self) -> Self {
        match self {
            Self::B8 | Self::B16 | Self::B32 => Self::B32,
            Self::B64 => Self::B64,
        }
    }

    /// Its index in the table, as a block names it.
    #[must_use]
    pub const fn id(self) -> SizeId {
        SizeId(self as u8)
    }
}

impl Mode {
    /// How an access in this mode updates its base.
    const fn writeback(self) -> Writeback {
        match self {
            Self::Pre | Self::PairPre => Writeback::Pre,
            Self::Post | Self::PairPost => Writeback::Post,
            _ => Writeback::No,
        }
    }
}

impl Choice {
    /// How many choices of this kind there are.
    const fn count(self) -> usize {
        match self {
            Self::Barrier => Barrier::ALL.len(),
            Self::Sys => SysReg::ALL.len(),
        }
    }
}

impl Kind {
    /// The kind of named choice this position takes, if it takes one.
    #[must_use]
    pub const fn choice(self) -> Option<Choice> {
        match self {
            Self::Barrier => Some(Choice::Barrier),
            Self::Sys => Some(Choice::Sys),
            _ => None,
        }
    }

    /// Whether a register at this position has the instruction's width.
    const fn sized(self) -> bool {
        matches!(self, Self::Gpr(_, Width::Inst) | Self::Fpr(Width::Inst) | Self::Shifted(_))
    }

    /// The class of register this position takes, if it takes one, and the width it reads it at
    /// in an instruction of `size`, if a fixed one.
    const fn register(self, size: Size) -> Option<(Class, Option<Size>)> {
        match self {
            Self::Gpr(_, width) => Some((Class::General, Some(width.at(size).named()))),
            Self::Shifted(_) => Some((Class::General, Some(size.named()))),
            Self::Extended => Some((Class::General, None)),
            Self::Fpr(width) => Some((Class::Fp, Some(width.at(size)))),
            _ => None,
        }
    }

    /// Whether this position of an instruction of `size` takes `given`.
    fn fits<V: Clone, P: Clone, S: Copy>(self, size: Size, given: &Given<V, P, S>) -> Result<(), Why> {
        // A register, or a value bound to one, fits only a position taking a register.
        if let Some(class) = given.class() {
            let Some((want, width)) = self.register(size) else { return Err(Why::Kind) };
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
            (_, Given::Choice { of, index })
                if self.choice().is_some_and(|c| c as u8 == of.0 && usize::from(*index) < c.count()) => {}
            (Self::Mem(mode, _), Given::Mem { mem, writeback }) if mode.writeback() == *writeback => {
                Arm64::addressable(mem, self).map_err(|_| Why::Kind)?;
            }
            (_, Given::Choice { .. } | Given::Mem { .. }) => return Err(Why::Kind),
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
    /// The first register of its class.
    fn bound(&mut self, class: ClassId) -> RegRef {
        RegRef { class, number: 0 }
    }

    fn line(&self, _: Line) -> Operand<S> {
        Operand::Label(Label::new(0))
    }

    fn via(&self, _: S) -> Via {
        Via::Direct
    }
}

impl Takes for Kind {
    fn cond(self) -> bool {
        matches!(self, Self::Cond(_))
    }

    fn list(self) -> bool {
        false
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
    let positions = positions(kinds, fixed, givens)?;
    // A width the spelling gives is the instruction's; its registers are checked against it.
    let sizes = form.sizes.iter().map(|s| s.size.id());
    let size = Size::of(width(&positions, fixed, Kind::sized, sizes)?);
    if form.at(size).is_none() {
        return Err(Misfit::Width);
    }
    for p in &positions {
        p.kind.fits(size, &p.given).map_err(|why| p.misfit(why))?;
    }
    Ok(Fit { size: size.id(), operands: positions.into_iter().map(|p| p.given.operand()).collect() })
}
