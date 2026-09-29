//! Thumb, the instruction set of Arm's M profile and of the A profile's Thumb state. Every
//! instruction is declared once, in `lang.arch.thumb`, as its forms, from which `generated` is
//! made: a base encoding, a halfword or two, the widths it is defined at with the bits each sets,
//! and the operand kinds with the bit each operand's field starts at. Callers ask which forms fit
//! before choosing operands, as arm64's do; the assembler (`vun-asm-thumb`) emits the encoding of
//! the first form an instruction's operands fit, the listing writes the same form's text, and
//! sema checks blocks of inline assembly against the same forms.

mod fit;
mod fixup;
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

pub use fixup::{Fixup, PC_AHEAD, bytes};
pub use inst::Inst;
pub use operand::{List, Mem, Operand, Shift, ShiftKind};
pub use reg::{Barrier, Cc, Choice, Class, Mask, Reg, Size, Special, Status, V, VName};
pub use table::{
    Access, Arg, Branch, Encoding, Feature, Fields, Form, Halves, Imm, Kind, List as Lists, Mode, Need, Op,
    Place, Processor, Regs, Sized, Specials, Then, Width,
};

/// The instruction set.
#[derive(Clone, Copy, Debug)]
pub struct Thumb;

impl vun_isa::Isa for Thumb {
    type Inst<S: Copy> = Inst<S>;
    type Fixup = Fixup;
    type Feature = Feature;

    fn patch(fixup: Fixup, code: &mut [u8], at: u64, target: u64) -> Result<(), Unpatched> {
        fixup.patch(code, at, target)
    }
}

impl Branch {
    /// The field a reference to a label writes.
    #[must_use]
    pub const fn fixup(self) -> Fixup {
        match self {
            Self::NarrowCond => Fixup::NarrowCond,
            Self::Narrow => Fixup::Narrow,
            Self::Compare => Fixup::Compare,
            Self::Cond => Fixup::Cond,
            Self::Jump | Self::Call => Fixup::Jump,
        }
    }
}
