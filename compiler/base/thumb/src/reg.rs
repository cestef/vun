//! Registers, conditions, widths and named choices, by their declarations' order.

pub use crate::generated::{Barrier, Cc, Choice, Class, Mask, Reg, Size, Special};
use crate::generated::{CONDITIONS, NAMES};

impl Reg {
    /// Its name, as the table's variant writes it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        NAMES[self as usize][0]
    }
}

impl Cc {
    /// Its name, as a mnemonic's suffix writes it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        CONDITIONS[self as usize]
    }
}
