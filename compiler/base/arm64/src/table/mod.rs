//! The instructions, each with its forms: the base word, the widths a form is defined at and the
//! bits each sets, and the operand kinds with where their fields go. The table is generated from
//! `lang.arch.arm64`; the encoder, the printer and form selection (TPDE's encoding candidates: the
//! first form whose operands fit) all read it, so an instruction's encoding and its text cannot
//! disagree.

mod kind;

pub use crate::generated::Op;
pub use vun_isa::word::Access;
pub(crate) use kind::{ARITH_BITS, HALF_BITS};
pub use kind::{Branch, Cond, Imm, Kind, Mode, R31, Shifts, Width};

use crate::operand::Operand;
use crate::reg::{Reg, Size};

/// A form of an instruction: its base word, the widths it is defined at with the bits each sets,
/// its operands with the bit each field starts at, and what it reads and writes beyond them.
pub type Form = vun_isa::word::Form<Kind, Size, Reg>;
pub type Arg = vun_isa::word::Arg<Kind>;
pub type Sized = vun_isa::word::Sized<Size>;

impl Op {
    /// The first form taking `operands` at `size`, and the word it encodes them to.
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
