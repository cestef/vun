//! What an operand position takes, and the field an operand there encodes to, relative to the
//! bit its argument starts at. Whether an operand fits is whether it encodes: one function
//! answers both, so a form never takes what it cannot encode.

use vun_isa::Via;
use vun_isa::imm::{fits_signed, truncate};

use crate::Fixup;
use crate::bitmask::Bitmask;
use crate::operand::{Extend, Mem, Operand, Scale, ShiftKind};
use crate::reg::{Reg, Size};

/// What register 31 is at a position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum R31 {
    Zr,
    Sp,
}

impl R31 {
    #[must_use]
    pub fn takes(self, reg: Reg) -> bool {
        reg != match self {
            Self::Zr => Reg::Sp,
            Self::Sp => Reg::Zr,
        }
    }
}

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

/// An immediate encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Imm {
    /// 12 bits, or 12 bits shifted left by 12: `add`, `sub`.
    Arith,
    /// A [`Bitmask`]: the logical instructions.
    Bitmask,
    /// A value whose set bits lie in one aligned 16 bits (`movz`), written whole.
    Wide,
    /// A value whose complement is `Wide` (`movn`), written whole.
    WideNot,
    /// `Wide`, written as its 16 bits and their shift (`movk`).
    Half,
    /// A left shift by 1 to the width less one, as `ubfm`'s rotation and top bit.
    Lsl,
    /// A right shift by 1 to the width less one, as the rotation and top bit of `ubfm` or `sbfm`.
    Shr,
    /// 16 bits, unsigned.
    U16,
    /// Floating-point zero, which `fcmp` compares with.
    Zero,
}

/// The bits a `Wide` value is moved in.
pub(crate) const HALF_BITS: u32 = 16;
/// The bits of an arithmetic immediate, and the shift of its shifted form.
pub(crate) const ARITH_BITS: u32 = 12;
/// The most an extended register is shifted by.
const EXTEND_SHIFT: u8 = 4;

impl Imm {
    /// The field holding `value` in an operation of `bits`, if it fits.
    #[must_use]
    pub fn encode(self, value: i64, bits: u32) -> Option<u32> {
        let v = truncate(value, bits);
        let amount = || u32::try_from(v).ok().filter(|n| (1..bits).contains(n));
        match self {
            Self::Arith if v < 1 << ARITH_BITS => u32::try_from(v).ok(),
            Self::Arith if v.trailing_zeros() >= ARITH_BITS && v >> ARITH_BITS < 1 << ARITH_BITS => {
                u32::try_from(1 << ARITH_BITS | v >> ARITH_BITS).ok()
            }
            Self::Arith => None,
            Self::Bitmask => Bitmask::of(value, bits).map(Bitmask::bits),
            Self::Wide | Self::Half => Self::wide(v, bits),
            Self::WideNot => Self::wide(truncate(!value, bits), bits),
            Self::Lsl => amount().map(|n| ((bits - n) % bits) << 6 | (bits - 1 - n)),
            Self::Shr => amount().map(|n| n << 6 | (bits - 1)),
            Self::U16 => u16::try_from(value).ok().map(u32::from),
            Self::Zero => (value == 0).then_some(0),
        }
    }

    /// `hw:imm16`: the 16 bits holding every set bit of `v`, and which 16 they are.
    fn wide(v: u64, bits: u32) -> Option<u32> {
        let hw = if v == 0 { 0 } else { v.trailing_zeros() / HALF_BITS };
        let imm16 = u32::try_from(v >> (hw * HALF_BITS)).ok().filter(|&i| i < 1 << HALF_BITS)?;
        (hw < bits / HALF_BITS).then_some(hw << HALF_BITS | imm16)
    }
}

/// An addressing mode: which memory operands a position takes, and how it encodes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// `[base, #offset]`, the offset a multiple of the access width, below 4096 of them.
    Scaled,
    /// `[base, #offset]`, the offset -256 to 255 (`ldur`, `stur`).
    Unscaled,
    /// `[base, #offset]!`, -256 to 255.
    Pre,
    /// `[base], #offset`, -256 to 255.
    Post,
    /// `[base, index, extend]`, the index 64 bits or 32 extended, scaled by 1 or the access width.
    Index,
    /// `[base, #offset]` of a pair, the offset a multiple of the access width, -64 to 63 of them.
    Pair,
    PairPre,
    PairPost,
    /// `[base]`: exclusive and atomic accesses.
    Base,
    /// `[base, :lo12:target]`, through the global offset table for a 64-bit load only.
    Lo12,
}

/// The bits of each offset's field.
const SCALED: u32 = 12;
const UNSCALED: u32 = 9;
const PAIR: u32 = 7;

impl Mode {
    /// The fields of `mem` from the base register's (bit 5), in an access of `access`.
    #[must_use]
    pub fn encode<S>(self, mem: &Mem<S>, access: Size) -> Option<u32> {
        // An offset as a count of accesses, if it is a whole number of them.
        let count = |offset: i64| Some(offset >> access.shift()).filter(|c| c << access.shift() == offset);
        let signed = |v: i64, bits: u32, at: u32| {
            fits_signed(v, bits, 64)
                .then(|| truncate(v, bits))
                .and_then(|f| u32::try_from(f).ok())
                .map(|f| f << at)
        };
        let fields = match (self, mem) {
            (Self::Scaled, Mem::Off { offset, .. }) => count(*offset)
                .and_then(|c| u32::try_from(c).ok())
                .filter(|&c| c < 1 << SCALED)
                .map(|c| c << 5),
            (Self::Unscaled, Mem::Off { offset, .. })
            | (Self::Pre, Mem::Pre { offset, .. })
            | (Self::Post, Mem::Post { offset, .. }) => signed(*offset, UNSCALED, 7),
            (Self::Index, Mem::Index { index, extend, scale, .. }) => {
                let extends = matches!(extend, Extend::Uxtw | Extend::Uxtx | Extend::Sxtw | Extend::Sxtx);
                (extends && *index != Reg::Sp).then(|| {
                    u32::from(index.number()) << 11
                        | extend.number() << 8
                        | u32::from(*scale == Scale::Access) << 7
                })
            }
            (Self::Pair, Mem::Off { offset, .. })
            | (Self::PairPre, Mem::Pre { offset, .. })
            | (Self::PairPost, Mem::Post { offset, .. }) => count(*offset).and_then(|c| signed(c, PAIR, 10)),
            (Self::Base, Mem::Off { offset: 0, .. }) => Some(0),
            (Self::Lo12, Mem::Lo12 { via, .. }) => (*via == Via::Direct || access == Size::B64).then_some(0),
            _ => None,
        }?;
        let base = mem.base();
        (base != Reg::Zr).then(|| fields | u32::from(base.number()))
    }
}

/// Which shifts a shifted register operand takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shifts {
    /// `lsl`, `lsr`, `asr`: arithmetic.
    Arith,
    /// Those and `ror`: the logical instructions.
    Logic,
}

impl Shifts {
    #[must_use]
    pub fn takes(self, kind: ShiftKind) -> bool {
        self == Self::Logic || kind != ShiftKind::Ror
    }
}

/// How a condition is written and encoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cond {
    /// As the mnemonic's suffix: `b.eq`.
    Suffix,
    Operand,
    /// As an operand, encoded as the condition it is not: `cset` is `csinc` of the inverse.
    Inverted,
}

/// A branch's field, and the relocation that fills it with a symbol's address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Branch {
    Jump26,
    Call26,
    Cond19,
}

impl Branch {
    #[must_use]
    pub const fn fixup(self) -> Fixup {
        match self {
            Self::Jump26 | Self::Call26 => Fixup::Imm26,
            Self::Cond19 => Fixup::Imm19,
        }
    }
}

/// What an operand position takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Gpr(R31, Width),
    Fpr(Width),
    Imm(Imm),
    /// A register, shifted or not, at the instruction's width; 31 is the zero register.
    Shifted(Shifts),
    /// A register, extended or read as it is; 31 is the zero register.
    Extended,
    /// Memory accessed at a width.
    Mem(Mode, Width),
    Cond(Cond),
    /// A label, or a symbol the linker places.
    Target(Branch),
    /// A symbol's page, or its entry's in the global offset table (`adrp`).
    Page,
    /// A symbol's low 12 bits (`add`).
    Lo12,
    /// What a barrier orders (`dmb`).
    Barrier,
    /// A system register (`mrs`, `msr`).
    Sys,
}

impl<S> vun_isa::word::Field<Operand<S>, Size> for Kind {
    fn encode(self, operand: &Operand<S>, size: Size) -> Option<u32> {
        let bits = size.bits();
        let reg = |r: &Reg| (*r != Reg::Sp).then(|| u32::from(r.number()) << 6);
        match (self, operand) {
            (Self::Gpr(r31, _), Operand::Reg(r)) => r31.takes(*r).then(|| u32::from(r.number())),
            (Self::Fpr(_), Operand::V(v)) => (v.0 < 32).then(|| u32::from(v.0)),
            (Self::Imm(imm), Operand::Imm(v)) => imm.encode(*v, bits),
            (Self::Shifted(_), Operand::Reg(r)) => reg(r),
            (Self::Shifted(shifts), Operand::Shifted(r, shift)) => {
                let fits = shifts.takes(shift.kind) && (1..bits).contains(&u32::from(shift.amount));
                reg(r).filter(|_| fits).map(|r| (shift.kind as u32) << 12 | r | u32::from(shift.amount))
            }
            (Self::Extended, Operand::Reg(r)) => reg(r).map(|r| Extend::identity(size).number() << 3 | r),
            (Self::Extended, Operand::Extended(r, extend, amount)) => {
                let fits =
                    *extend != Extend::identity(size) && extend.reads() <= size && *amount <= EXTEND_SHIFT;
                reg(r).filter(|_| fits).map(|r| extend.number() << 3 | r | u32::from(*amount))
            }
            (Self::Mem(mode, width), Operand::Mem(mem)) => mode.encode(mem, width.at(size)),
            (Self::Cond(Cond::Inverted), Operand::Cond(cc)) => Some(u32::from(cc.not().number())),
            (Self::Cond(_), Operand::Cond(cc)) => Some(u32::from(cc.number())),
            (Self::Barrier, Operand::Barrier(b)) => Some(b.field()),
            (Self::Sys, Operand::Sys(r)) => Some(r.field()),
            (Self::Target(_), Operand::Label(_) | Operand::Sym(_))
            | (Self::Page, Operand::Page(..))
            | (Self::Lo12, Operand::Lo12(..)) => Some(0),
            _ => None,
        }
    }
}
