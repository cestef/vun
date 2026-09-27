//! Fitting a line's operands to an x86-64 form: the instruction's width is the one its registers
//! at `r`, `rm` and `m` positions have, else the form's only one; each position takes what the
//! encoder's `Kind::takes` does, a register of the position's class and width, and an immediate
//! the operation's bits hold.

use vun_inline::{FormId, OpId, SizeId};
use vun_isa::imm::held;
use vun_isa::{Blocks, Fit, Fixed, Given, Label, Misfit, Why, positions};

use crate::{Base, Cc, Class, Kind, Mem, Op, Operand, Reg, Size, X86_64, Xmm};

impl Size {
    /// Its index in the table, as a block names it.
    #[must_use]
    pub const fn id(self) -> SizeId {
        SizeId(self as u8)
    }
}

impl Kind {
    /// Whether an operand at this position has the instruction's width.
    const fn sized(self) -> bool {
        matches!(self, Self::R | Self::Rm | Self::M)
    }

    /// The class of register this position takes, if it takes one.
    const fn class(self) -> Option<Class> {
        match self {
            Self::R | Self::Rn(_) | Self::Rm | Self::Rmn(_) | Self::Cl => Some(Class::General),
            Self::X | Self::Xmn(_) => Some(Class::Sse),
            Self::M | Self::I8 | Self::Iz | Self::U32 | Self::I64 | Self::Rel | Self::Cond => None,
        }
    }

    /// Whether this position of an instruction of `size` takes `given`.
    fn fits<V, P, S: Copy>(self, size: Size, given: &Given<V, P, S>) -> Result<(), Why> {
        if let (Some(class), Some(want)) = (given.class(), self.class()) {
            if Class::ALL[usize::from(class.0)] != want {
                return Err(Why::Class);
            }
            if let (Some(width), Some(want)) = (given.size(), self.width(size))
                && Size::of(width) != want
            {
                return Err(Why::Kind);
            }
        }
        // The operand as the encoder sees it: a register a binding is given stands for any of
        // its class.
        let operand = match given {
            Given::Reg { reg, .. } => {
                X86_64::general(*reg).map_or(Operand::Xmm(Xmm(reg.number)), Operand::Reg)
            }
            Given::Bound { class, .. } => match Class::ALL[usize::from(class.0)] {
                Class::General => Operand::Reg(Reg::Rax),
                Class::Sse => Operand::Xmm(Xmm(0)),
            },
            Given::Imm(Some(v)) => Operand::Imm(*v),
            Given::Imm(None) => return Err(if self.is_imm() { Why::Wide } else { Why::Kind }),
            Given::Cond(c) => Operand::Cond(Cc::ALL[usize::from(c.0)]),
            Given::Mem(_) => Operand::Mem(Mem { base: Base::Reg(Reg::Rax), disp: 0 }),
            Given::Label(_) => Operand::Label(Label::new(0)),
            Given::Symbol(s) => Operand::Sym(*s),
            Given::Choice(_) => return Err(Why::Kind),
        };
        match operand {
            _ if !self.takes(size, &operand) => {
                Err(if self.is_imm() && matches!(given, Given::Imm(_)) { Why::Wide } else { Why::Kind })
            }
            Operand::Imm(v) if !held(v, size.bits()) => Err(Why::Wide),
            _ => Ok(()),
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
    let positions = positions(form.kinds(), fixed, |k| k == Kind::Cond, givens)?;
    let mut known = None;
    for p in positions.iter().filter(|p| p.kind.sized()) {
        let Some(width) = p.given.size() else { continue };
        if known.is_some_and(|k| k != width) {
            return Err(p.misfit(Why::Kind));
        }
        known = Some(width);
    }
    let mut sizes = Size::ALL.into_iter().filter(|&s| form.sizes.contains(s));
    let size = match known.or(fixed.size) {
        Some(width) => Size::of(width),
        None if sizes.clone().count() == 1 || !positions.iter().any(|p| p.kind.sized()) => {
            sizes.next().ok_or(Misfit::Width)?
        }
        None => return Err(Misfit::Unsized),
    };
    if !form.sizes.contains(size) {
        return Err(Misfit::Width);
    }
    for p in &positions {
        p.kind.fits(size, &p.given).map_err(|why| p.misfit(why))?;
    }
    Ok(Fit { size: size.id(), operands: positions.into_iter().map(|p| p.given.operand()).collect() })
}
