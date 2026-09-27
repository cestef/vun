//! The instructions, each with its forms: the base word, the widths a form is defined at and the
//! bits each sets, and the operand kinds with where their fields go. The table is generated from
//! `lang.arch.arm64`; the encoder, the printer and form selection (TPDE's encoding candidates: the
//! first form whose operands fit) all read it, so an instruction's encoding and its text cannot
//! disagree.

mod form;
mod kind;

pub use crate::generated::Op;
pub use form::{Access, Arg, Form, Sized};
pub(crate) use kind::{ARITH_BITS, HALF_BITS};
pub use kind::{Branch, Cond, Imm, Kind, Mode, R31, Shifts, Width};

use crate::operand::Operand;
use crate::reg::Size;

impl Op {
    /// The first form taking `operands` at `size`, and the word it encodes them to.
    #[must_use]
    pub fn select<S>(self, size: Size, operands: &[Operand<S>]) -> Option<(&'static Form, u32)> {
        self.forms().iter().find_map(|f| f.encode(size, operands).map(|word| (f, word)))
    }

    /// The first form taking `operands` at `size`.
    #[must_use]
    pub fn form<S>(self, size: Size, operands: &[Operand<S>]) -> Option<&'static Form> {
        self.select(size, operands).map(|(form, _)| form)
    }
}
