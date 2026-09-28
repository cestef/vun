//! What an operand position takes, and the field an operand there encodes to, relative to the
//! bit its argument starts at. Whether an operand fits is whether it encodes: one function
//! answers both, so a form never takes what it cannot encode.

use vun_isa::word::Field;

use super::imm::Imm;
use crate::generated::{Branch, Fields, List as Lists, Mode, Regs, Then};
use crate::operand::{List, Operand, ShiftKind};
use crate::reg::{Reg, Size, Status, V};

/// The width of an operand: the instruction's, or its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Width {
    Inst,
    Is(Size),
}

impl Width {
    /// Its width in an instruction of `size`.
    #[must_use]
    pub const fn at(self, size: Size) -> Size {
        match self {
            Self::Inst => size,
            Self::Is(s) => s,
        }
    }
}

/// What an operand position takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Gpr(Regs),
    /// Any register in a low register's field with a fourth bit at 7: a 16-bit `add` or `mov`.
    Split,
    Fpr(Width, Fields),
    Imm(Imm),
    /// A register, shifted or not: the amount in `imm3:imm2`, the shift's kind at 4, the register
    /// at 0.
    Shifted,
    /// Memory accessed at a width.
    Mem(Mode, Width),
    /// A condition, as the mnemonic's suffix.
    Cond,
    /// The condition an `it` block's first instruction holds on, as an operand, and those of the
    /// instructions after it.
    Then(Then),
    /// A label, or a symbol the linker places.
    Target(Branch),
    /// The low or high 16 bits of a symbol's address: `movw`, `movt`.
    Low,
    High,
    List(Lists),
    Special,
    Status(Status),
    Mask,
    Barrier,
}

impl Regs {
    /// Whether a position of these registers names `reg`.
    #[must_use]
    pub const fn takes(self, reg: Reg) -> bool {
        match self {
            Self::Low => reg.is_low(),
            Self::General => !matches!(reg, Reg::Sp | Reg::Pc),
            Self::Sp => !matches!(reg, Reg::Pc),
            Self::Any => true,
            Self::OnlySp => matches!(reg, Reg::Sp),
        }
    }

    /// The field naming `reg`: none for `sp` alone.
    const fn field(self, reg: Reg) -> u32 {
        match self {
            Self::OnlySp => 0,
            _ => reg.number() as u32,
        }
    }
}

impl Fields {
    /// The four bits' position and the fifth's.
    const fn at(self) -> (u32, u32) {
        match self {
            Self::D => (12, 22),
            Self::N => (16, 7),
            Self::M => (0, 5),
        }
    }

    /// The fields of register `v` at `size`: a single-precision number's low bit is the fifth
    /// bit, a double-precision one's high bit is.
    fn encode(self, v: V, size: Size) -> Option<u32> {
        let n = u32::from(v.at(size)?);
        let (four, fifth) = if size == Size::B64 { (n & 0xf, n >> 4) } else { (n >> 1, n & 1) };
        let (at4, at1) = self.at();
        Some(four << at4 | fifth << at1)
    }
}

/// The double-precision registers a list names.
const DOUBLES: u32 = 16;

impl Lists {
    /// The field of `list`, if this position takes it.
    fn encode(self, list: List) -> Option<u32> {
        const LOW: u16 = 0xff;
        let (sp, lr, pc) = (1 << Reg::Sp.number(), 1 << Reg::Lr.number(), 1 << Reg::Pc.number());
        match (self, list) {
            (Self::Push, List::Regs(regs)) if regs & !(LOW | lr) == 0 => {
                Some(u32::from(regs & LOW) | u32::from(regs & lr != 0) << 8)
            }
            (Self::Pop, List::Regs(regs)) if regs & !(LOW | pc) == 0 => {
                Some(u32::from(regs & LOW) | u32::from(regs & pc != 0) << 8)
            }
            // A 32-bit list names two registers at least, never `sp`, and not both `lr` and `pc`.
            (Self::Wide, List::Regs(regs)) if regs.count_ones() >= 2 && regs & (sp | pc) == 0 => {
                Some(u32::from(regs))
            }
            (Self::WidePc, List::Regs(regs))
                if regs.count_ones() >= 2 && regs & sp == 0 && regs & (lr | pc) != lr | pc =>
            {
                Some(u32::from(regs))
            }
            (Self::Doubles, List::Doubles { first, count }) => {
                let (d, count) = (u32::from(first.at(Size::B64)?), u32::from(count));
                if count == 0 || d + count > DOUBLES {
                    return None;
                }
                Fields::D.encode(first, Size::B64).map(|f| f | (count * 2))
            }
            _ => None,
        }
    }
}

impl Then {
    /// `firstcond` at 4 and the mask below it: a bit per later instruction, the condition's lowest
    /// for the same condition and its opposite for the other, then a bit ending the block.
    fn encode(self, cc: crate::Cc) -> u32 {
        let (cond, low) = (u32::from(cc.number()), u32::from(cc.number()) & 1);
        let mask = match self {
            Self::None => 0b1000,
            Self::T => low << 3 | 0b100,
            Self::E => (low ^ 1) << 3 | 0b100,
        };
        cond << 4 | mask
    }
}

impl<S> Field<Operand<S>, Size> for Kind {
    fn encode(self, operand: &Operand<S>, size: Size) -> Option<u32> {
        let general = |r: &Reg| Regs::General.takes(*r).then(|| u32::from(r.number()));
        match (self, operand) {
            (Self::Gpr(regs), Operand::Reg(r)) => regs.takes(*r).then(|| regs.field(*r)),
            (Self::Split, Operand::Reg(r)) => {
                let n = u32::from(r.number());
                Some((n & 7) | (n >> 3) << 7)
            }
            (Self::Fpr(width, fields), Operand::V(v)) => fields.encode(*v, width.at(size)),
            (Self::Imm(imm), Operand::Imm(v)) => imm.encode(*v),
            (Self::Shifted, Operand::Reg(r)) => general(r),
            (Self::Shifted, Operand::Shifted(r, shift)) => {
                let amount = u32::from(shift.amount);
                let place = match shift.kind {
                    ShiftKind::Lsl | ShiftKind::Ror => (1..32).contains(&amount).then_some(amount),
                    ShiftKind::Lsr | ShiftKind::Asr => (1..=32).contains(&amount).then_some(amount % 32),
                }?;
                let fields = crate::generated::Place::Wide.shift(place);
                general(r).map(|r| r | (shift.kind as u32) << 4 | fields)
            }
            (Self::Mem(mode, width), Operand::Mem(mem)) => mode.encode(mem, width.at(size)),
            (Self::Cond, Operand::Cond(cc)) => (*cc != crate::Cc::Al).then(|| u32::from(cc.number())),
            (Self::Then(then), Operand::Cond(cc)) => (*cc != crate::Cc::Al).then(|| then.encode(*cc)),
            (
                Self::Target(Branch::Jump | Branch::Call | Branch::Cond),
                Operand::Label(_) | Operand::Sym(_),
            )
            | (Self::Target(_), Operand::Label(_))
            | (Self::Low, Operand::Low(_))
            | (Self::High, Operand::High(_)) => Some(0),
            (Self::List(lists), Operand::List(list)) => lists.encode(*list),
            (Self::Special, Operand::Special(s)) => Some(s.field()),
            (Self::Status(want), Operand::Status(s)) => (want == *s).then_some(0),
            (Self::Mask, Operand::Mask(m)) => Some(m.field()),
            (Self::Barrier, Operand::Barrier(b)) => Some(b.field()),
            _ => None,
        }
    }
}
