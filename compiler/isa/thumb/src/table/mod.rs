//! The instructions, each with its forms: the base encoding, the widths a form is defined at and
//! the bits each sets, and the operand kinds with where their fields go. The table is generated
//! from `lang.arch.thumb`; the encoder, the printer and form selection (the first form whose
//! operands fit) all read it, so an instruction's encoding and its text cannot disagree.

mod imm;
mod kind;
mod mem;
mod syntax;

pub use crate::generated::{Branch, Fields, List, Mode, Op, Place, Regs};
pub use imm::Imm;
pub use kind::{Kind, Width};
pub use vun_isa::word::Access;

use crate::operand::Operand;
use crate::reg::{Reg, Size};

/// A form of an instruction: its base encoding, the widths it is defined at with the bits each
/// sets, its operands with the bit each field starts at, and what it reads and writes beyond them.
pub type Form = vun_isa::word::Form<Kind, Size, Reg>;
pub type Arg = vun_isa::word::Arg<Kind>;
pub type Sized = vun_isa::word::Sized<Size>;

/// The widest halfword a 16-bit encoding is: every 32-bit one's first halfword is above it.
const NARROW: u32 = 0xffff;

/// Whether a form's encoding is a halfword or two.
pub trait Halves {
    /// Whether it is 32 bits: its first halfword says so, as it starts `0b11101`, `0b11110` or
    /// `0b11111`.
    fn wide(&self) -> bool;

    /// Its bytes.
    fn bytes(&self) -> u64 {
        if self.wide() { 4 } else { 2 }
    }
}

impl Halves for Form {
    fn wide(&self) -> bool {
        self.base > NARROW
    }
}

impl Op {
    /// The first form taking `operands` at `size`, and the encoding it gives them.
    #[must_use]
    pub fn select<S>(self, size: Size, operands: &[Operand<S>]) -> Option<(&'static Form, u32)> {
        Form::select(self.forms(), size, operands)
    }

    /// The first form taking `operands` at `size`.
    #[must_use]
    pub fn form<S>(self, size: Size, operands: &[Operand<S>]) -> Option<&'static Form> {
        self.select(size, operands).map(|(form, _)| form)
    }
}
