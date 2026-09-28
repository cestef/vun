//! Addressing modes: which memory operands a position takes, and their fields. A 16-bit form's
//! fields are relative to bit 0, the base at 3; a 32-bit form's base is at 16, and its bits saying
//! how the offset is applied are set here: `P` (at 10 for a byte offset, 24 for a pair), `U`, the
//! offset added (9 or 23), and `W`, the base written back (8 or 21).

use crate::generated::Mode;
use crate::operand::Mem;
use crate::reg::{Reg, Size};

/// The bits of a 16-bit offset, of the stack pointer's, of a 32-bit one, and of a byte's.
const NARROW: u32 = 5;
const NARROW_SP: u32 = 8;
const OFFSET: u32 = 12;
const BYTE: u32 = 8;

/// Where the bits of a byte offset go: `P`, `U`, `W`.
const BYTE_PUW: (u32, u32, u32) = (10, 9, 8);
/// Where they go for a pair and a floating-point access.
const WORDS_PUW: (u32, u32, u32) = (24, 23, 21);

/// How an offset is applied: before the access, added, written back.
struct Puw {
    pre: bool,
    add: bool,
    back: bool,
}

impl Puw {
    const fn bits(self, (p, u, w): (u32, u32, u32)) -> u32 {
        (self.pre as u32) << p | (self.add as u32) << u | (self.back as u32) << w
    }

    /// How `mem` applies its offset, the offset's size, and its base.
    fn of(mem: &Mem) -> Option<(Self, u32, Reg)> {
        let (base, offset, pre, back) = match *mem {
            Mem::Off { base, offset } => (base, offset, true, false),
            Mem::Pre { base, offset } => (base, offset, true, true),
            Mem::Post { base, offset } => (base, offset, false, true),
            Mem::Index { .. } => return None,
        };
        let size = u32::try_from(offset.unsigned_abs()).ok()?;
        Some((Self { pre, add: offset >= 0, back }, size, base))
    }
}

/// `n` as a count of `unit`s below `2^bits`, if it is one.
fn count(n: u32, unit: u32, bits: u32) -> Option<u32> {
    (n % unit == 0 && n / unit < 1 << bits).then_some(n / unit)
}

impl Mode {
    /// The fields of `mem` in an access of `access`.
    #[must_use]
    pub fn encode(self, mem: &Mem, access: Size) -> Option<u32> {
        let base = |r: Reg| (r != Reg::Pc).then(|| u32::from(r.number()) << 16);
        match (self, *mem) {
            (Self::Narrow, Mem::Off { base, offset }) if base.is_low() => {
                let n = count(u32::try_from(offset).ok()?, access.bytes(), NARROW)?;
                Some(n << 6 | u32::from(base.number()) << 3)
            }
            (Self::NarrowSp, Mem::Off { base: Reg::Sp, offset }) => {
                count(u32::try_from(offset).ok()?, access.bytes(), NARROW_SP)
            }
            (Self::NarrowIndex, Mem::Index { base, index, shift: 0 }) if base.is_low() && index.is_low() => {
                Some(u32::from(index.number()) << 6 | u32::from(base.number()) << 3)
            }
            (Self::Offset, Mem::Off { base: b, offset }) => {
                let offset = u32::try_from(offset).ok().filter(|&o| o < 1 << OFFSET)?;
                base(b).map(|b| b | offset)
            }
            (Self::Index, Mem::Index { base: b, index, shift }) if shift < 4 && !matches!(index, Reg::Sp | Reg::Pc) => {
                base(b).map(|b| b | u32::from(shift) << 4 | u32::from(index.number()))
            }
            (Self::Byte, Mem::Off { offset, .. })
            | (Self::BytePre, Mem::Pre { offset, .. })
            | (Self::BytePost, Mem::Post { offset, .. }) => {
                let (puw, n, b) = Puw::of(mem)?;
                // A positive offset not written back is `ldrt`, an access of another privilege.
                let fits = n < 1 << BYTE && (puw.back || offset < 0);
                fits.then(|| base(b).map(|b| b | puw.bits(BYTE_PUW) | n))?
            }
            (Self::Pair, Mem::Off { .. }) | (Self::PairPre, Mem::Pre { .. }) | (Self::PairPost, Mem::Post { .. }) => {
                let (puw, n, b) = Puw::of(mem)?;
                let n = count(n, 4, BYTE)?;
                base(b).map(|b| b | puw.bits(WORDS_PUW) | n)
            }
            (Self::Fp, Mem::Off { base: b, offset }) => {
                let n = count(u32::try_from(offset.unsigned_abs()).ok()?, 4, BYTE)?;
                base(b).map(|b| b | u32::from(offset >= 0) << WORDS_PUW.1 | n)
            }
            (Self::Exclusive, Mem::Off { base: b, offset }) => {
                let n = count(u32::try_from(offset).ok()?, 4, BYTE)?;
                base(b).map(|b| b | n)
            }
            _ => None,
        }
    }
}
