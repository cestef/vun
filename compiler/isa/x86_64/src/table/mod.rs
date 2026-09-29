//! The instructions, each with its forms: the operand kinds a form takes, the widths it is
//! defined at, how it is encoded, and what it reads and writes. The table is generated from
//! `lang.arch.x86_64`; the encoder, the printer and form selection (TPDE's encoding candidates:
//! the first form whose operands fit) all read it, so an instruction's encoding and its text
//! cannot disagree.

mod form;

pub use crate::generated::{Feature, Op};
pub use form::{Access, Form, Kind, Layout, Opcode, Prefix, Sizes, Slot, W};

use crate::operand::Operand;
use crate::reg::{Reg, Size};

/// What a form needs of a processor beyond x86-64's baseline.
pub type Need = vun_isa::Need<Feature, Size>;
/// What a processor has of x86-64 beyond its baseline.
pub type Processor = vun_isa::Processor<Feature>;

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

    /// The first form taking `operands` at `size` that `processor` has; `None` where no form
    /// takes them.
    ///
    /// # Errors
    /// What the first taking them needs that `processor` lacks, where it has none of them.
    pub fn choose<S>(
        self,
        processor: Processor,
        size: Size,
        operands: &[Operand<S>],
    ) -> Result<Option<&'static Form>, Need> {
        let taking = self.forms().iter().filter(|f| f.takes(size, operands));
        processor.choose(taking.map(|f| (f, f.needs(size))))
    }

    /// Whether `processor` has a form of it at `size`, whatever its operands.
    #[must_use]
    pub fn available(self, processor: Processor, size: Size) -> bool {
        self.forms().iter().any(|f| f.sizes.contains(size) && processor.lacks(f.needs(size)).is_none())
    }
}
