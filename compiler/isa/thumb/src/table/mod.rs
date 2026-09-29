//! The instructions, each with its forms: the base encoding, the widths a form is defined at and
//! the bits each sets, and the operand kinds with where their fields go. The table is generated
//! from `lang.arch.thumb`; the encoder, the printer and form selection (the first form whose
//! operands fit) all read it, so an instruction's encoding and its text cannot disagree.

mod imm;
mod kind;
mod mem;
mod syntax;

pub use crate::generated::{Branch, Encoding, Feature, Fields, List, Mode, Op, Place, Regs, Then};
pub use imm::Imm;
pub use kind::{Kind, Specials, Width};
pub use vun_isa::word::Access;

use crate::operand::Operand;
use crate::reg::{Reg, Size};

/// A form of an instruction: its base encoding, the widths it is defined at with the bits each
/// sets, its operands with the bit each field starts at, and what it reads and writes beyond them.
pub type Form = vun_isa::word::Form<Kind, Size, Reg, Need>;
pub type Arg = vun_isa::word::Arg<Kind>;
pub type Sized = vun_isa::word::Sized<Size, Need>;

/// What a form, or a width of it, needs of a processor beyond Thumb.
pub type Need = vun_isa::Need<Feature, Size>;
/// What a processor has of Thumb beyond what every one has.
pub type Processor = vun_isa::Processor<Feature>;

/// The widest halfword a 16-bit encoding is: every 32-bit one's first halfword is above it.
const NARROW: u32 = 0xffff;

/// Whether a form's encoding is a halfword or two.
pub trait Halves {
    /// The width of its encoding: 32 bits where its first halfword says so, as it starts
    /// `0b11101`, `0b11110` or `0b11111`.
    fn encoding(&self) -> Encoding;

    /// Whether it is 32 bits.
    fn wide(&self) -> bool {
        self.encoding() == Encoding::W
    }

    /// Its bytes.
    fn bytes(&self) -> u64 {
        if self.wide() { 4 } else { 2 }
    }
}

impl Halves for Form {
    fn encoding(&self) -> Encoding {
        if self.base > NARROW { Encoding::W } else { Encoding::N }
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
