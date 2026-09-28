//! Fitting a line's operands to a Thumb form: the instruction's width is the one its mnemonic's
//! spelling gives (`vadd.f64`), else its floating-point registers' (`d0`), else the form's first;
//! each position takes what the encoder's `Kind::encode` does, over the operand the backends make
//! of it, a register of the position's class and width, and an immediate a 32-bit operation holds.

use vun_inline::{FormId, Line, OpId, SizeId};
use vun_isa::imm::held;
use vun_isa::word::Field;
use vun_isa::{
    Blocks, Fit, Fixed, Given, Label, Misfit, Place, Takes, Via, Why, Writeback, positions, width,
};

use crate::{Choice, Class, Kind, List, Mode, Op, Operand, Reg, Size, Thumb, Width};

/// The bits of every operation on general registers.
const WORD_BITS: u32 = 32;

impl Size {
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
            Self::BytePre | Self::PairPre => Writeback::Pre,
            Self::BytePost | Self::PairPost => Writeback::Post,
            _ => Writeback::No,
        }
    }
}

impl Choice {
    /// How many choices of this kind there are.
    const fn count(self) -> usize {
        match self {
            Self::Mask => crate::Mask::ALL.len(),
            Self::Barrier => crate::Barrier::ALL.len(),
        }
    }
}

impl Kind {
    /// The kind of named choice this position takes, if it takes one.
    #[must_use]
    pub const fn choice(self) -> Option<Choice> {
        match self {
            Self::Mask => Some(Choice::Mask),
            Self::Barrier => Some(Choice::Barrier),
            _ => None,
        }
    }

    /// Whether a register at this position has the instruction's width.
    const fn sized(self) -> bool {
        matches!(self, Self::Fpr(Width::Inst, _))
    }

    /// The class of register this position takes, if it takes one, and the width it reads it at
    /// in an instruction of `size`, if a fixed one.
    const fn register(self, size: Size) -> Option<(Class, Option<Size>)> {
        match self {
            Self::Gpr(_) | Self::Split | Self::Shifted => Some((Class::General, Some(Size::B32))),
            Self::Fpr(width, _) => Some((Class::Fp, Some(width.at(size)))),
            Self::Special => Some((Class::Special, None)),
            Self::Status(_) => Some((Class::Status, None)),
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
            (_, Given::Choice { of, index })
                if self.choice().is_some_and(|c| c as u8 == of.0 && usize::from(*index) < c.count()) => {}
            (Self::Mem(mode, _), Given::Mem { mem, writeback }) if mode.writeback() == *writeback => {
                Thumb::addressable(mem, self).map_err(|_| Why::Kind)?;
            }
            // A list of floating-point registers names double-precision ones (`d8, d9`).
            (Self::List(_), Given::List(regs))
                if List::of(&regs.iter().map(|&(r, _)| r).collect::<Vec<_>>()).is_some()
                    && regs.iter().all(|&(r, size)| {
                        Class::ALL[usize::from(r.class.0)] != Class::Fp
                            || size.map(Size::of) == Some(Size::B64)
                    }) => {}
            (_, Given::Choice { .. } | Given::Mem { .. } | Given::List(_)) => return Err(Why::Kind),
            _ => {}
        }
        let operand = Thumb::operand(&given.clone().operand(), self, &mut Representative);
        match (self.encode(&operand, size), given) {
            (None, Given::Imm(_)) if matches!(self, Self::Imm(_)) => Err(Why::Wide),
            (None, _) => Err(Why::Kind),
            (Some(_), Given::Imm(Some(v))) if !held(*v, WORD_BITS) => Err(Why::Wide),
            (Some(_), _) => Ok(()),
        }
    }
}

/// The operands of a line as the encoder sees them before registers are given: a binding's
/// register stands for any general one, so no position taking low registers alone takes it.
struct Representative;

impl<S: Copy> Place<Thumb, S> for Representative {
    fn bound(&mut self) -> Reg {
        Reg::R8
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
        matches!(self, Self::Cond)
    }

    fn list(self) -> bool {
        matches!(self, Self::List(_))
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
    let positions = positions(form.kinds(), fixed, givens)?;
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
