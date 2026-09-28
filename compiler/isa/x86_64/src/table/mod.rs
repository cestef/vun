//! The instructions, each with its forms: the operand kinds a form takes, the widths it is
//! defined at, how it is encoded, and what it reads and writes. The table is generated from
//! `lang.arch.x86_64`; the encoder, the printer and form selection (TPDE's encoding candidates:
//! the first form whose operands fit) all read it, so an instruction's encoding and its text
//! cannot disagree.

mod form;

pub use crate::generated::Op;
pub use form::{Access, Form, Kind, Layout, Opcode, Prefix, Sizes, Slot, W};

use crate::operand::Operand;
use crate::reg::{Reg, Size};

impl Op {
    /// The registers it reads or writes without naming them, whichever form encodes it.
    #[must_use]
    pub const fn implicit(self) -> &'static [Reg] {
        self.forms()[0].implicit
    }

    /// The first form taking `operands` at `size`.
    #[must_use]
    pub fn form<S>(self, size: Size, operands: &[Operand<S>]) -> Option<&'static Form> {
        self.forms().iter().find(|f| f.takes(size, operands))
    }
}
