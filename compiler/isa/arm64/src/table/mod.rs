//! The instructions, each with its forms: the base word, the widths a form is defined at and the
//! bits each sets, and the operand kinds with where their fields go. The table is generated from
//! `lang.arch.arm64`; the encoder, the printer and form selection (TPDE's encoding candidates: the
//! first form whose operands fit) all read it, so an instruction's encoding and its text cannot
//! disagree.

mod kind;

pub use crate::generated::{Feature, Op};
pub(crate) use kind::{ARITH_BITS, HALF_BITS};
pub use kind::{Branch, Cond, Imm, Kind, Mode, R31, Shifts, Width};
pub use vun_isa::word::Access;

use crate::operand::Operand;
use crate::reg::{Reg, Size};

/// A form of an instruction: its base word, the widths it is defined at with the bits each sets,
/// its operands with the bit each field starts at, and what it reads and writes beyond them.
pub type Form = vun_isa::word::Form<Kind, Size, Reg, Need>;
pub type Arg = vun_isa::word::Arg<Kind>;
pub type Sized = vun_isa::word::Sized<Size, Need>;

/// What a form needs of a processor beyond Armv8.0.
pub type Need = vun_isa::Need<Feature, Size>;
/// What a processor has of A64 beyond Armv8.0.
pub type Processor = vun_isa::Processor<Feature>;

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

    /// The first form taking `operands` at `size` that `processor` has, and the word it encodes
    /// them to; `None` where no form takes them.
    ///
    /// # Errors
    /// What the first taking them needs that `processor` lacks, where it has none of them.
    pub fn choose<S>(
        self,
        processor: Processor,
        size: Size,
        operands: &[Operand<S>],
    ) -> Result<Option<(&'static Form, u32)>, Need> {
        Form::choose(self.forms(), processor, size, operands)
    }

    /// Whether `processor` has a form of it at `size`, whatever its operands.
    #[must_use]
    pub fn available(self, processor: Processor, size: Size) -> bool {
        Form::available(self.forms(), processor, size)
    }
}
