//! Relocatable objects as data: sections of bytes, symbols and relocations, built by a backend
//! and written once in the target's object format (`Object::write`: ELF-64 or Mach-O 64-bit,
//! little-endian).

#[cfg(test)]
mod tests;
mod write;

use vun_index::{IndexVec, index};

index! {
    pub struct SectionId;
    pub struct SymbolId;
}

/// What the linker writes at a relocation, whatever the machine; each machine numbers the kinds
/// it has (`Machine::relocation`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelocKind {
    /// The symbol's address, 64 bits.
    Abs64,
    /// The symbol's address, 32 bits: an offset into a section of debugging information.
    Abs32,
    /// The symbol's address relative to the field, 32 bits.
    Pc32,
    /// A call's target relative to the field, through the procedure linkage table if needed.
    Call32,
    /// The address of the symbol's entry in the global offset table, relative to the field.
    GotPc32,
    /// The 4 KiB page of the symbol's address relative to the field's page, 21 bits (`adrp`).
    Page21,
    /// The symbol's address within its page, 12 bits, as an `add` reads them.
    PageOffset12,
    /// The symbol's address within its page, as a load or store of a byte reads it.
    PageOffset8,
    /// The same, scaled by 2 for a load or store of 16 bits.
    PageOffset16,
    /// The same, scaled by 4 for a load or store of 32 bits.
    PageOffset32,
    /// The same, scaled by 8 for a load or store of 64 bits.
    PageOffset64,
    /// The page of the symbol's entry in the global offset table relative to the field's page.
    GotPage21,
    /// The address of the symbol's entry within its page, as a 64-bit load reads it.
    GotPageOffset64,
    /// A jump's target relative to the field, 26 bits counting instructions.
    Jump26,
    /// A call's target relative to the field, 26 bits counting instructions, through the
    /// procedure linkage table if needed.
    Call26,
    /// A conditional branch's target relative to the field, 19 bits counting instructions.
    Branch19,
}

/// What a section holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Contents {
    Code,
    /// Data the program only reads.
    ReadOnly,
    /// Data only read once the loader has written the addresses in it.
    Relocated,
    Writable,
    /// The mark that the stack need not be executable.
    StackNote,
    /// Information for debuggers, which the program does not load.
    Debug,
}

pub struct Section {
    pub name: Vec<u8>,
    pub contents: Contents,
    pub bytes: Vec<u8>,
    pub align: u64,
    pub relocs: Vec<Reloc>,
}

/// The address of `symbol` plus `addend`, written at `offset` of a section as `kind` says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reloc {
    pub offset: u64,
    pub symbol: SymbolId,
    pub kind: RelocKind,
    pub addend: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolKind {
    Func,
    Object,
    /// A section itself, which relocations into it name.
    Section,
    /// Defined elsewhere, of a kind not said.
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Binding {
    /// Seen by this object only.
    Local,
    Global,
}

pub struct Symbol {
    pub name: Vec<u8>,
    /// Where it is defined; `None` for one defined elsewhere.
    pub section: Option<SectionId>,
    pub value: u64,
    pub size: u64,
    pub kind: SymbolKind,
    pub binding: Binding,
}

/// The machine an object is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Machine {
    X86_64,
    Aarch64,
}

/// The file format an object is written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Elf,
    MachO,
}

pub struct Object {
    pub machine: Machine,
    pub format: Format,
    pub sections: IndexVec<SectionId, Section>,
    pub symbols: IndexVec<SymbolId, Symbol>,
}

impl Object {
    #[must_use]
    pub fn new(machine: Machine, format: Format) -> Self {
        Self { machine, format, sections: IndexVec::new(), symbols: IndexVec::new() }
    }

    /// Adds a section holding `contents`, empty.
    pub fn section(&mut self, name: &[u8], contents: Contents) -> SectionId {
        self.sections.push(Section {
            name: name.to_vec(),
            contents,
            bytes: Vec::new(),
            align: 1,
            relocs: Vec::new(),
        })
    }

    /// The symbol standing for `section`, made the first time it is asked for.
    pub fn section_symbol(&mut self, section: SectionId) -> SymbolId {
        let found =
            self.symbols.iter().find(|(_, s)| s.kind == SymbolKind::Section && s.section == Some(section));
        if let Some((id, _)) = found {
            return id;
        }
        self.symbols.push(Symbol {
            name: Vec::new(),
            section: Some(section),
            value: 0,
            size: 0,
            kind: SymbolKind::Section,
            binding: Binding::Local,
        })
    }

    /// Appends `bytes` to `section` at `align`: the offset they start at.
    ///
    /// # Panics
    /// Panics if the section outgrows memory.
    pub fn append(&mut self, section: SectionId, bytes: &[u8], align: u64) -> u64 {
        let s = &mut self.sections[section];
        let offset = (s.bytes.len() as u64).next_multiple_of(align.max(1));
        s.bytes.resize(usize::try_from(offset).expect("sections fit memory"), 0);
        s.bytes.extend_from_slice(bytes);
        s.align = s.align.max(align);
        offset
    }
}
