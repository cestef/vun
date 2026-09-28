//! The machines objects are for, and what each fixes of them: the width of an address.

use crate::RelocKind;

/// The machine an object is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Machine {
    X86_64,
    Aarch64,
    /// 32-bit Arm, under the calling convention's use of floating-point registers.
    Arm(Float),
}

/// Where a 32-bit Arm convention passes floating-point values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Float {
    /// In general registers.
    Soft,
    /// In floating-point registers.
    Hard,
}

impl Machine {
    /// The width of an address.
    #[must_use]
    pub const fn pointer_bits(self) -> u32 {
        match self {
            Self::X86_64 | Self::Aarch64 => 64,
            Self::Arm(_) => 32,
        }
    }

    /// The bytes of an address.
    #[must_use]
    pub const fn address_bytes(self) -> u8 {
        match self.pointer_bits() {
            64 => 8,
            _ => 4,
        }
    }

    /// The relocation of a word of data holding an address.
    #[must_use]
    pub const fn address(self) -> RelocKind {
        match self.pointer_bits() {
            64 => RelocKind::Abs64,
            _ => RelocKind::Abs32,
        }
    }
}
