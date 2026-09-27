//! A form of an instruction: its base word, the widths it is defined at with the bits each sets,
//! its operands, each a kind, the bit its field starts at and what the form does with it, and the
//! registers and flags it reads and writes beyond them.

use super::kind::Kind;
use crate::operand::Operand;
use crate::reg::{Reg, Size};

/// What a form does with an operand, or with the flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
    ReadWrite,
    /// Nothing but compute its address.
    Address,
}

/// An operand position: what it takes, where its field goes, and what the form does with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arg {
    pub kind: Kind,
    pub at: u8,
    /// Where the same field goes again, for an alias reading one register in two fields (`cneg`
    /// is `csneg` of a register and itself).
    pub again: Option<u8>,
    pub access: Access,
}

impl Arg {
    #[must_use]
    pub const fn new(kind: Kind, at: u8, access: Access) -> Self {
        Self { kind, at, again: None, access }
    }

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
    /// The general registers it reads and writes without naming them (the link register of `bl`).
    pub reads: &'static [Reg],
    pub writes: &'static [Reg],
    /// What it does with the flags, if anything.
    pub flags: Option<Access>,
}

impl Form {
    #[must_use]
    pub const fn new(base: u32, sizes: &'static [Sized], args: &'static [Arg]) -> Self {
        Self { base, sizes, args, name: None, reads: &[], writes: &[], flags: None }
    }

    #[must_use]
    pub const fn named(self, name: &'static str) -> Self {
        Self { name: Some(name), ..self }
    }

    #[must_use]
    pub const fn reads(self, reads: &'static [Reg]) -> Self {
        Self { reads, ..self }
    }

    #[must_use]
    pub const fn writes(self, writes: &'static [Reg]) -> Self {
        Self { writes, ..self }
    }

    #[must_use]
    pub const fn flags(self, flags: Access) -> Self {
        Self { flags: Some(flags), ..self }
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
