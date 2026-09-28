//! The numbers ELF gives machines, their flags, relocations, symbol kinds and bindings, and the
//! names of mapping symbols.

use crate::{Binding, Float, Machine, Mapping, RelocKind, SymbolKind};

/// `EM_*`.
const EM_ARM: u16 = 40;
const EM_X86_64: u16 = 62;
const EM_AARCH64: u16 = 183;

/// `EF_ARM_*`: the version of the Arm EABI, and where its convention passes floats.
const EF_ARM_EABI_VER5: u32 = 0x0500_0000;
const EF_ARM_ABI_FLOAT_SOFT: u32 = 0x200;
const EF_ARM_ABI_FLOAT_HARD: u32 = 0x400;

/// `R_X86_64_*`.
const R_X86_64_64: u32 = 1;
const R_X86_64_PC32: u32 = 2;
const R_X86_64_PLT32: u32 = 4;
const R_X86_64_GOTPCREL: u32 = 9;
const R_X86_64_32: u32 = 10;

/// `R_ARM_*`.
const R_ARM_ABS32: u32 = 2;
const R_ARM_REL32: u32 = 3;
const R_ARM_THM_CALL: u32 = 10;
const R_ARM_THM_JUMP24: u32 = 30;
const R_ARM_THM_MOVW_ABS_NC: u32 = 47;
const R_ARM_THM_MOVT_ABS: u32 = 48;
const R_ARM_THM_JUMP19: u32 = 51;

/// `R_AARCH64_*`.
const R_AARCH64_ABS64: u32 = 257;
const R_AARCH64_ABS32: u32 = 258;
const R_AARCH64_PREL32: u32 = 261;
const R_AARCH64_ADR_PREL_PG_HI21: u32 = 275;
const R_AARCH64_ADD_ABS_LO12_NC: u32 = 277;
const R_AARCH64_LDST8_ABS_LO12_NC: u32 = 278;
const R_AARCH64_CONDBR19: u32 = 280;
const R_AARCH64_JUMP26: u32 = 282;
const R_AARCH64_CALL26: u32 = 283;
const R_AARCH64_LDST16_ABS_LO12_NC: u32 = 284;
const R_AARCH64_LDST32_ABS_LO12_NC: u32 = 285;
const R_AARCH64_LDST64_ABS_LO12_NC: u32 = 286;
const R_AARCH64_ADR_GOT_PAGE: u32 = 311;
const R_AARCH64_LD64_GOT_LO12_NC: u32 = 312;

/// `STT_*`.
const STT_NOTYPE: u8 = 0;
const STT_OBJECT: u8 = 1;
const STT_FUNC: u8 = 2;
const STT_SECTION: u8 = 3;

/// `STB_*`.
const STB_LOCAL: u8 = 0;
const STB_GLOBAL: u8 = 1;

impl Machine {
    /// Its `EM_*` number.
    pub(super) const fn elf(self) -> u16 {
        match self {
            Self::X86_64 => EM_X86_64,
            Self::Aarch64 => EM_AARCH64,
            Self::Arm(_) => EM_ARM,
        }
    }

    /// Its `e_flags`.
    pub(super) const fn elf_flags(self) -> u32 {
        match self {
            Self::X86_64 | Self::Aarch64 => 0,
            Self::Arm(Float::Soft) => EF_ARM_EABI_VER5 | EF_ARM_ABI_FLOAT_SOFT,
            Self::Arm(Float::Hard) => EF_ARM_EABI_VER5 | EF_ARM_ABI_FLOAT_HARD,
        }
    }

    /// The value ELF gives a symbol of `kind` defined at `value`: a Thumb function's has bit 0
    /// set, which tells linkers the state its code runs in (Arm ELF ABI).
    pub(super) const fn elf_value(self, kind: SymbolKind, value: u64) -> u64 {
        match (self, kind) {
            (Self::Arm(_), SymbolKind::Func) => value | 1,
            _ => value,
        }
    }

    /// The number of the relocation `kind`, if the machine has it.
    pub(super) const fn elf_relocation(self, kind: RelocKind) -> Option<u32> {
        Some(match (self, kind) {
            (Self::X86_64, RelocKind::Abs64) => R_X86_64_64,
            (Self::X86_64, RelocKind::Pc32) => R_X86_64_PC32,
            (Self::X86_64, RelocKind::Call32) => R_X86_64_PLT32,
            (Self::X86_64, RelocKind::GotPc32) => R_X86_64_GOTPCREL,
            (Self::X86_64, RelocKind::Abs32) => R_X86_64_32,
            (Self::Aarch64, RelocKind::Abs64) => R_AARCH64_ABS64,
            (Self::Aarch64, RelocKind::Abs32) => R_AARCH64_ABS32,
            (Self::Aarch64, RelocKind::Pc32) => R_AARCH64_PREL32,
            (Self::Aarch64, RelocKind::Page21) => R_AARCH64_ADR_PREL_PG_HI21,
            (Self::Aarch64, RelocKind::PageOffset12) => R_AARCH64_ADD_ABS_LO12_NC,
            (Self::Aarch64, RelocKind::PageOffset8) => R_AARCH64_LDST8_ABS_LO12_NC,
            (Self::Aarch64, RelocKind::Branch19) => R_AARCH64_CONDBR19,
            (Self::Aarch64, RelocKind::Jump26) => R_AARCH64_JUMP26,
            (Self::Aarch64, RelocKind::Call26) => R_AARCH64_CALL26,
            (Self::Aarch64, RelocKind::PageOffset16) => R_AARCH64_LDST16_ABS_LO12_NC,
            (Self::Aarch64, RelocKind::PageOffset32) => R_AARCH64_LDST32_ABS_LO12_NC,
            (Self::Aarch64, RelocKind::PageOffset64) => R_AARCH64_LDST64_ABS_LO12_NC,
            (Self::Aarch64, RelocKind::GotPage21) => R_AARCH64_ADR_GOT_PAGE,
            (Self::Aarch64, RelocKind::GotPageOffset64) => R_AARCH64_LD64_GOT_LO12_NC,
            (Self::Arm(_), RelocKind::Abs32) => R_ARM_ABS32,
            (Self::Arm(_), RelocKind::Pc32) => R_ARM_REL32,
            (Self::Arm(_), RelocKind::ThumbCall) => R_ARM_THM_CALL,
            (Self::Arm(_), RelocKind::ThumbJump24) => R_ARM_THM_JUMP24,
            (Self::Arm(_), RelocKind::ThumbJump19) => R_ARM_THM_JUMP19,
            (Self::Arm(_), RelocKind::ThumbLow16) => R_ARM_THM_MOVW_ABS_NC,
            (Self::Arm(_), RelocKind::ThumbHigh16) => R_ARM_THM_MOVT_ABS,
            _ => return None,
        })
    }
}

impl SymbolKind {
    /// Its `STT_*` number.
    pub(super) const fn elf(self) -> u8 {
        match self {
            Self::Unknown | Self::Mapping(_) => STT_NOTYPE,
            Self::Object => STT_OBJECT,
            Self::Func => STT_FUNC,
            Self::Section => STT_SECTION,
        }
    }
}

impl Binding {
    /// Its `STB_*` number.
    pub(super) const fn elf(self) -> u8 {
        match self {
            Self::Local => STB_LOCAL,
            Self::Global => STB_GLOBAL,
        }
    }
}

impl Mapping {
    /// The name of its mapping symbol, which the Arm ELF ABI fixes.
    pub(super) const fn elf(self) -> &'static [u8] {
        match self {
            Self::Thumb => b"$t",
            Self::Data => b"$d",
        }
    }
}
