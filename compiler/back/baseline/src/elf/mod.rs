//! ELF relocatable objects (ELF-64, little-endian): sections of bytes, symbols and relocations,
//! built as data and written once (`write`).

mod write;

use vun_index::{IndexVec, index};

index! {
    pub struct SectionId;
    pub struct SymbolId;
}

/// A relocation of x86-64, by what the linker writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelocKind {
    /// The symbol's address, 64 bits.
    Abs64,
    /// The symbol's address relative to the field, 32 bits.
    Pc32,
    /// A call's target relative to the field, through the procedure linkage table if needed.
    Plt32,
    /// The address of the symbol's entry in the global offset table, relative to the field.
    GotPc32,
}

impl RelocKind {
    /// Its `R_X86_64_*` number.
    #[must_use]
    pub const fn number(self) -> u32 {
        match self {
            Self::Abs64 => 1,
            Self::Pc32 => 2,
            Self::Plt32 => 4,
            Self::GotPc32 => 9,
        }
    }
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
}

impl Machine {
    const fn number(self) -> u16 {
        match self {
            Self::X86_64 => 62,
        }
    }
}

pub struct Object {
    pub machine: Machine,
    pub sections: IndexVec<SectionId, Section>,
    pub symbols: IndexVec<SymbolId, Symbol>,
}

impl Object {
    #[must_use]
    pub fn new(machine: Machine) -> Self {
        Self { machine, sections: IndexVec::new(), symbols: IndexVec::new() }
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
