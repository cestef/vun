//! The machines objects are for, and what each fixes of them: the width of an address.

use crate::{Mapping, RelocKind, SymbolKind};

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

    /// The instructions its code is, where objects mark it: Thumb for 32-bit Arm.
    #[must_use]
    pub const fn code(self) -> Option<Mapping> {
        match self {
            Self::X86_64 | Self::Aarch64 => None,
            Self::Arm(_) => Some(Mapping::Thumb),
        }
    }

    /// The address code reaches a symbol of `kind` defined at `value` by: a Thumb function's has
    /// bit 0 set, which tells a branch the state it runs in (Arm ELF ABI).
    #[must_use]
    pub const fn reached(self, kind: SymbolKind, value: u64) -> u64 {
        match (self.code(), kind) {
            (Some(Mapping::Thumb), SymbolKind::Func) => value | 1,
            _ => value,
        }
    }

    /// Whether linkers read a relocation's addend from the field it fills, not from the entry
    /// (the Arm ELF ABI's `REL`: GNU ld ignores an entry's addend there). Relocations then name
    /// the symbol itself, and the fields hold their addends.
    #[must_use]
    pub const fn implicit_addends(self) -> bool {
        matches!(self, Self::Arm(_))
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
