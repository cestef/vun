//! What ELF numbers of an object's machine, symbols and relocations: each value mapped to
//! `vun_elf`'s.

use vun_elf::{aarch64, arm, x86_64};

use crate::{Binding, Float, Machine, Mapping, RelocKind, SymbolKind};

impl Machine {
    pub(super) const fn elf(self) -> vun_elf::Machine {
        match self {
            Self::X86_64 => vun_elf::Machine::X86_64,
            Self::Aarch64 => vun_elf::Machine::Aarch64,
            Self::Arm(_) => vun_elf::Machine::Arm,
        }
    }

    /// Its file's flags: for 32-bit Arm, the EABI's version and where functions pass floats.
    pub(super) const fn elf_flags(self) -> u32 {
        match self {
            Self::X86_64 | Self::Aarch64 => 0,
            Self::Arm(Float::Soft) => arm::EABI5 | arm::FloatAbi::Soft.number(),
            Self::Arm(Float::Hard) => arm::EABI5 | arm::FloatAbi::Hard.number(),
        }
    }

    /// The number of the relocation `kind`, if the machine has it.
    pub(super) const fn elf_relocation(self, kind: RelocKind) -> Option<u32> {
        Some(match (self, kind) {
            (Self::X86_64, RelocKind::Abs64) => x86_64::Reloc::Abs64.number(),
            (Self::X86_64, RelocKind::Pc32) => x86_64::Reloc::Pc32.number(),
            (Self::X86_64, RelocKind::Call32) => x86_64::Reloc::Plt32.number(),
            (Self::X86_64, RelocKind::GotPc32) => x86_64::Reloc::GotPcRel.number(),
            (Self::X86_64, RelocKind::Abs32) => x86_64::Reloc::Abs32.number(),
            (Self::Aarch64, RelocKind::Abs64) => aarch64::Reloc::Abs64.number(),
            (Self::Aarch64, RelocKind::Abs32) => aarch64::Reloc::Abs32.number(),
            (Self::Aarch64, RelocKind::Pc32) => aarch64::Reloc::Prel32.number(),
            (Self::Aarch64, RelocKind::Page21) => aarch64::Reloc::AdrPrelPgHi21.number(),
            (Self::Aarch64, RelocKind::PageOffset12) => aarch64::Reloc::AddAbsLo12Nc.number(),
            (Self::Aarch64, RelocKind::PageOffset8) => aarch64::Reloc::Ldst8AbsLo12Nc.number(),
            (Self::Aarch64, RelocKind::Branch19) => aarch64::Reloc::CondBr19.number(),
            (Self::Aarch64, RelocKind::Jump26) => aarch64::Reloc::Jump26.number(),
            (Self::Aarch64, RelocKind::Call26) => aarch64::Reloc::Call26.number(),
            (Self::Aarch64, RelocKind::PageOffset16) => aarch64::Reloc::Ldst16AbsLo12Nc.number(),
            (Self::Aarch64, RelocKind::PageOffset32) => aarch64::Reloc::Ldst32AbsLo12Nc.number(),
            (Self::Aarch64, RelocKind::PageOffset64) => aarch64::Reloc::Ldst64AbsLo12Nc.number(),
            (Self::Aarch64, RelocKind::GotPage21) => aarch64::Reloc::AdrGotPage.number(),
            (Self::Aarch64, RelocKind::GotPageOffset64) => aarch64::Reloc::Ld64GotLo12Nc.number(),
            (Self::Arm(_), RelocKind::Abs32) => arm::Reloc::Abs32.number(),
            (Self::Arm(_), RelocKind::Pc32) => arm::Reloc::Rel32.number(),
            (Self::Arm(_), RelocKind::ThumbCall) => arm::Reloc::ThmCall.number(),
            (Self::Arm(_), RelocKind::ThumbJump24) => arm::Reloc::ThmJump24.number(),
            (Self::Arm(_), RelocKind::ThumbJump19) => arm::Reloc::ThmJump19.number(),
            (Self::Arm(_), RelocKind::ThumbLow16) => arm::Reloc::ThmMovwAbsNc.number(),
            (Self::Arm(_), RelocKind::ThumbHigh16) => arm::Reloc::ThmMovtAbs.number(),
            _ => return None,
        })
    }
}

impl SymbolKind {
    pub(super) const fn elf(self) -> vun_elf::SymbolKind {
        match self {
            Self::Unknown | Self::Mapping(_) => vun_elf::SymbolKind::NoType,
            Self::Object => vun_elf::SymbolKind::Object,
            Self::Func => vun_elf::SymbolKind::Func,
            Self::Section => vun_elf::SymbolKind::Section,
        }
    }
}

impl Binding {
    pub(super) const fn elf(self) -> vun_elf::Binding {
        match self {
            Self::Local => vun_elf::Binding::Local,
            Self::Global => vun_elf::Binding::Global,
        }
    }
}

impl Mapping {
    /// The name of its mapping symbol.
    pub(super) const fn elf(self) -> &'static [u8] {
        match self {
            Self::Thumb => arm::THUMB,
            Self::Data => arm::DATA,
        }
    }
}
