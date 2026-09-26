//! Relocations as Mach-O writes them for arm64: against a symbol, a section's offset or address,
//! an addend in place for a word and as a relocation of its own for an instruction's field.

use super::symbols::Order;
use super::{
    ADDEND, BRANCH26, GOT_LOAD_PAGE21, GOT_LOAD_PAGEOFF12, PAGE21, PAGEOFF12, Placed, Put, UNSIGNED, WORD32,
    WORD64,
};
use crate::{Machine, Object, Reloc, RelocKind, SymbolKind};

/// How a relocation is written: its type, whether it counts from the field, its length.
struct Kind {
    ty: u32,
    pcrel: bool,
    length: u32,
}

/// The Mach-O relocation of `kind` on `machine`, if it has one.
const fn relocation(machine: Machine, kind: RelocKind) -> Option<Kind> {
    let (ty, pcrel, length) = match (machine, kind) {
        (Machine::Aarch64, RelocKind::Abs64) => (UNSIGNED, false, WORD64),
        (Machine::Aarch64, RelocKind::Abs32) => (UNSIGNED, false, WORD32),
        (Machine::Aarch64, RelocKind::Call26 | RelocKind::Jump26) => (BRANCH26, true, WORD32),
        (Machine::Aarch64, RelocKind::Page21) => (PAGE21, true, WORD32),
        (
            Machine::Aarch64,
            RelocKind::PageOffset12
            | RelocKind::PageOffset8
            | RelocKind::PageOffset16
            | RelocKind::PageOffset32
            | RelocKind::PageOffset64,
        ) => (PAGEOFF12, false, WORD32),
        (Machine::Aarch64, RelocKind::GotPage21) => (GOT_LOAD_PAGE21, true, WORD32),
        (Machine::Aarch64, RelocKind::GotPageOffset64) => (GOT_LOAD_PAGEOFF12, false, WORD32),
        _ => return None,
    };
    Some(Kind { ty, pcrel, length })
}

impl Object {
    /// Writes `r`, a relocation of the section whose bytes are `bytes`: against its symbol, or
    /// for a section's symbol against the section, whose offsets into debugging sections need
    /// none. An addend is written in place for a word, as a relocation before it for a field
    /// of an instruction.
    pub(super) fn relocate(
        &self,
        r: &Reloc,
        placed: &[Placed],
        order: &Order,
        bytes: &mut [u8],
        out: &mut Vec<u8>,
    ) {
        let kind = relocation(self.machine, r.kind).expect("backends emit the relocations their machine has");
        let at = usize::try_from(r.offset).expect("objects fit memory");
        let symbol = &self.symbols[r.symbol];
        let (number, external, addend) = if symbol.kind == SymbolKind::Section {
            let section = symbol.section.expect("a section's symbol names it");
            let Some(k) = placed.iter().position(|p| p.id == section) else { return };
            if r.kind == RelocKind::Abs32 {
                bytes[at..at + 4].copy_from_slice(&u32::try_from(r.addend).expect("fits").to_le_bytes());
                return;
            }
            let addr = placed[k].addr.cast_signed() + r.addend;
            (u32::try_from(k + 1).expect("fits"), false, addr)
        } else {
            (order.number[r.symbol].expect("named symbols are numbered"), true, r.addend)
        };
        let address = u32::try_from(r.offset).expect("sections fit 4 GiB");
        if kind.ty == UNSIGNED {
            let width = if kind.length == WORD64 { 8 } else { 4 };
            bytes[at..at + width].copy_from_slice(&addend.to_le_bytes()[..width]);
        } else if addend != 0 {
            let addend = u32::try_from(addend).expect("addends of instructions fit 24 bits");
            out.put32(address);
            out.put32(addend | WORD32 << 25 | ADDEND << 28);
        }
        out.put32(address);
        out.put32(
            number
                | u32::from(kind.pcrel) << 24
                | kind.length << 25
                | u32::from(external) << 27
                | kind.ty << 28,
        );
    }
}
