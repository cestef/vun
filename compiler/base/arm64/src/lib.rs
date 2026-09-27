//! The 64-bit Arm (A64) instruction set. Every instruction is declared once, in
//! `lang.arch.arm64`, as its forms, from which `generated` is made: a base word, the widths it is
//! defined at with the bits each sets, and the operand kinds with the bit each operand's field
//! starts at. Callers ask which forms fit before choosing operands (an immediate or a register,
//! an offset or an address), as TPDE's snippet encoders do; the assembler (`vun-asm-arm64`)
//! emits the word of the first form an instruction's operands fit, the listing writes the same
//! form's text, and sema checks blocks of inline assembly against the same forms.

mod bitmask;
mod fit;
#[rustfmt::skip]
mod generated;
mod inline;
mod inst;
mod operand;
mod reg;
mod table;

#[cfg(test)]
mod tests;

use vun_isa::Unpatched;
use vun_isa::imm::{fits_signed, truncate};

pub use bitmask::Bitmask;
pub use inst::Inst;
pub use operand::{Barrier, Extend, Mem, Operand, Scale, Shift, ShiftKind, SysReg};
pub use reg::{Cc, Class, Reg, Size, V};
pub use table::{Access, Arg, Branch, Cond, Form, Imm, Kind, Mode, Op, R31, Shifts, Sized, Width};

/// The instruction set.
#[derive(Clone, Copy, Debug)]
pub struct Arm64;

/// The bytes of every instruction.
pub const INST_BYTES: u64 = 4;

/// A reference to a label: the distance from the referring instruction, in instructions, signed,
/// in the field its branch has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fixup {
    /// 26 bits from bit 0: `b`, `bl`.
    Imm26,
    /// 19 bits from bit 5: `b.cond`, `cbz`, `cbnz`.
    Imm19,
}

impl Fixup {
    /// The field's width and the bit it starts at.
    #[must_use]
    pub const fn field(self) -> (u32, u32) {
        match self {
            Self::Imm26 => (26, 0),
            Self::Imm19 => (19, 5),
        }
    }

    /// Whether a branch `distance` bytes away fits the field.
    #[must_use]
    pub const fn reaches(self, distance: i64) -> bool {
        distance % INST_BYTES.cast_signed() == 0
            && fits_signed(distance / INST_BYTES.cast_signed(), self.field().0, 64)
    }
}

impl vun_isa::Isa for Arm64 {
    type Inst<S: Copy> = Inst<S>;
    type Fixup = Fixup;

    fn patch(fixup: Fixup, code: &mut [u8], at: u64, target: u64) -> Result<(), Unpatched> {
        let distance = i128::from(target) - i128::from(at);
        let range = Unpatched::Range { at, distance: i64::try_from(distance).unwrap_or(i64::MAX) };
        let distance = i64::try_from(distance).map_err(|_| range)?;
        if !fixup.reaches(distance) {
            return Err(range);
        }
        let (bits, shift) = fixup.field();
        let field = u32::try_from(truncate(distance / INST_BYTES.cast_signed(), bits))
            .expect("the field is narrower");
        let at = usize::try_from(at).expect("code fits memory");
        let word: &mut [u8; 4] = (&mut code[at..at + size_of::<u32>()]).try_into().expect("a word");
        *word = (u32::from_le_bytes(*word) | field << shift).to_le_bytes();
        Ok(())
    }
}
