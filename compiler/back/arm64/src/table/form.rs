//! A form of an instruction: its base word, the widths it is defined at with the bits each sets,
//! and its operands, each a kind and the bit its field starts at.

use super::kind::Kind;
use crate::operand::Operand;
use crate::reg::Size;

/// Where registers go: the destination or transferred register, the first source or base, the
/// second source, and the third (the addend, or a pair's second register).
pub(super) const D: u8 = 0;
pub(super) const N: u8 = 5;
pub(super) const M: u8 = 16;
pub(super) const A: u8 = 10;

/// An operand position: what it takes and where its field goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arg {
    pub kind: Kind,
    pub at: u8,
    /// Where the same field goes again, for an alias reading one register twice (`cneg`).
    pub again: Option<u8>,
}

impl Kind {
    #[must_use]
    pub const fn at(self, at: u8) -> Arg {
        Arg { kind: self, at, again: None }
    }
}

impl Arg {
    #[must_use]
    pub const fn again(self, at: u8) -> Self {
        Self { again: Some(at), ..self }
    }
}

/// A width a form is defined at, the bits it sets, and the suffix it gives the mnemonic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sized {
    pub size: Size,
    pub bits: u32,
    pub suffix: &'static str,
}

impl Sized {
    #[must_use]
    pub const fn new(size: Size, bits: u32) -> Self {
        Self { size, bits, suffix: "" }
    }

    #[must_use]
    pub const fn suffix(self, suffix: &'static str) -> Self {
        Self { suffix, ..self }
    }
}

/// One form of an instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Form {
    pub base: u32,
    pub sizes: &'static [Sized],
    pub args: &'static [Arg],
    /// The mnemonic, where it is not the instruction's (`ldur`).
    pub name: Option<&'static str>,
}

impl Form {
    #[must_use]
    pub const fn new(base: u32, sizes: &'static [Sized], args: &'static [Arg]) -> Self {
        Self { base, sizes, args, name: None }
    }

    #[must_use]
    pub const fn named(self, name: &'static str) -> Self {
        Self { name: Some(name), ..self }
    }

    /// Its definition at `size`.
    #[must_use]
    pub fn at(&self, size: Size) -> Option<&'static Sized> {
        self.sizes.iter().find(|s| s.size == size)
    }

    /// The word encoding `operands` at `size`, if this form takes them.
    #[must_use]
    pub fn encode<S>(&self, size: Size, operands: &[Operand<S>]) -> Option<u32> {
        let sized = self.at(size)?;
        if self.args.len() != operands.len() {
            return None;
        }
        self.args.iter().zip(operands).try_fold(self.base | sized.bits, |word, (arg, operand)| {
            let field = arg.kind.encode(operand, size)?;
            Some(word | field << arg.at | arg.again.map_or(0, |at| field << at))
        })
    }

    /// Whether this form takes `operands` at `size`.
    #[must_use]
    pub fn takes<S>(&self, size: Size, operands: &[Operand<S>]) -> bool {
        self.encode(size, operands).is_some()
    }
}
