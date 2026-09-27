//! The instructions, each with its forms: the kinds of operands a form takes and what it does with
//! each, the widths it is defined at, and the special registers it reads and writes.

pub use crate::generated::{Access, Kind, Op};
use crate::reg::{Size, Special};

/// An operand position: what it takes, and what the form does with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot {
    pub kind: Kind,
    pub access: Access,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Form {
    pub slots: &'static [Slot],
    pub sizes: &'static [Size],
    /// The mnemonic, where it is not the instruction's.
    pub name: Option<&'static str>,
    pub reads: &'static [Special],
    pub writes: &'static [Special],
}

impl Form {
    /// The kinds of its operands, in order.
    pub fn kinds(&self) -> impl Iterator<Item = Kind> {
        self.slots.iter().map(|s| s.kind)
    }
}
