//! The relocations of compiled code as the object writer's.

use cranelift_codegen::binemit::Reloc;
use vun_object::RelocKind;

/// Each relocation Cranelift writes for the machines the backend lists, and what the linker
/// writes for it.
const PLACED: &[(Reloc, RelocKind)] = &[
    (Reloc::Abs8, RelocKind::Abs64),
    (Reloc::X86PCRel4, RelocKind::Pc32),
    (Reloc::X86CallPCRel4, RelocKind::Call32),
    (Reloc::X86CallPLTRel4, RelocKind::Call32),
    (Reloc::X86GOTPCRel4, RelocKind::GotPc32),
    (Reloc::ElfX86_64TlsGd, RelocKind::TlsGdPc32),
    (Reloc::Arm64Call, RelocKind::Call26),
    (Reloc::Aarch64AdrPrelPgHi21, RelocKind::Page21),
    (Reloc::Aarch64AddAbsLo12Nc, RelocKind::PageOffset12),
    (Reloc::Aarch64AdrGotPage21, RelocKind::GotPage21),
    (Reloc::Aarch64Ld64GotLo12Nc, RelocKind::GotPageOffset64),
    (Reloc::Aarch64TlsDescAdrPage21, RelocKind::TlsDescPage21),
    (Reloc::Aarch64TlsDescLd64Lo12, RelocKind::TlsDescPageOffset64),
    (Reloc::Aarch64TlsDescAddLo12, RelocKind::TlsDescPageOffset12),
    (Reloc::Aarch64TlsDescCall, RelocKind::TlsDescCall),
];

/// A relocation Cranelift writes, as the object writer names it.
pub(crate) trait Placed {
    /// What the linker writes, or none for a relocation these machines do not use.
    fn kind(self) -> Option<RelocKind>;
}

impl Placed for Reloc {
    fn kind(self) -> Option<RelocKind> {
        PLACED.iter().find(|(reloc, _)| *reloc == self).map(|&(_, kind)| kind)
    }
}
