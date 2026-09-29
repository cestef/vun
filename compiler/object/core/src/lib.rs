//! Relocatable objects as data: sections of bytes, symbols and relocations, built by a backend
//! and written once in the target's object format (`Object::write`: ELF of the machine's class, or
//! Mach-O 64-bit, little-endian).

mod machine;
#[cfg(test)]
mod tests;
mod write;

pub use machine::{Float, Machine};

use vun_index::{Idx, IndexVec, index};

index! {
    pub struct SectionId;
    pub struct SymbolId;
}

/// What the linker writes at a relocation, whatever the machine; each machine numbers the kinds
/// it has (`Machine::elf_relocation`, `Machine::macho_relocation`).
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
    /// The address of the symbol's TLS descriptor in the global offset table, relative to the
    /// field, 32 bits: storage per thread reached through a call.
    TlsDescPc32,
    /// A call through the symbol's TLS descriptor: marks the instruction, writing no field.
    TlsDescCall,
    /// The page of the symbol's TLS descriptor relative to the field's page, 21 bits (`adrp`).
    TlsDescPage21,
    /// The address of the symbol's TLS descriptor within its page, as a 64-bit load reads it.
    TlsDescPageOffset64,
    /// The same, as an `add` reads it.
    TlsDescPageOffset12,
    /// The symbol's offset from the thread pointer, 32 bits, in a word of data.
    ThreadOffset32,
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
    /// A Thumb `bl`'s target relative to the field, through the procedure linkage table if
    /// needed.
    ThumbCall,
    /// A Thumb `b.w`'s target relative to the field, 24 bits counting halfwords.
    ThumbJump24,
    /// A Thumb conditional `b.w`'s target relative to the field, 20 bits counting halfwords.
    ThumbJump19,
    /// The low 16 bits of the symbol's address, as a Thumb `movw` reads them.
    ThumbLow16,
    /// The high 16 bits of the symbol's address, as a Thumb `movt` reads them.
    ThumbHigh16,
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
    /// Data each thread has a copy of, starting as these bytes.
    PerThread,
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
    /// Data each thread has a copy of, by its offset in the program's storage per thread.
    Tls,
    /// Defined elsewhere, of a kind not said.
    Unknown,
    /// Where what a section holds changes, for disassemblers and linkers (`Object::map`).
    Mapping(Mapping),
}

/// What a section holds from a mapping symbol on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mapping {
    /// Thumb instructions.
    Thumb,
    /// Data, such as a literal pool among instructions.
    Data,
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

    /// The section named `name` holding `contents`, added the first time it is asked for.
    pub fn section_named(&mut self, name: &[u8], contents: Contents) -> SectionId {
        let found = self.sections.iter().find(|(_, s)| s.contents == contents && s.name == name);
        match found.map(|(id, _)| id) {
            Some(id) => id,
            None => self.section(name, contents),
        }
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

    /// Makes every relocation naming a symbol this object defines name its section instead, at
    /// the address the symbol is reached by: the object's own definitions are reached as they are
    /// here, never through another module's, which a shared library needs of every symbol it
    /// exports.
    ///
    /// # Panics
    /// Panics on a symbol at an offset past 2^63.
    pub fn bind_locally(&mut self) {
        let targets: Vec<Option<(SectionId, u64)>> = self
            .symbols
            .iter()
            .map(|(_, s)| {
                let reached = self.machine.reached(s.kind, s.value);
                s.section.filter(|_| s.kind != SymbolKind::Section).map(|at| (at, reached))
            })
            .collect();
        let mut sections = self.sections.iter().map(|(id, _)| id).collect::<Vec<_>>();
        sections.retain(|id| targets.iter().flatten().any(|(at, _)| at == id));
        let symbols: Vec<(SectionId, SymbolId)> =
            sections.into_iter().map(|s| (s, self.section_symbol(s))).collect();
        for (_, section) in self.sections.iter_mut() {
            for r in &mut section.relocs {
                let Some((at, value)) = targets.get(r.symbol.index()).copied().flatten() else { continue };
                let (_, symbol) =
                    symbols.iter().find(|(s, _)| *s == at).expect("each defining section has one");
                r.symbol = *symbol;
                r.addend += i64::try_from(value).expect("offsets fit 63 bits");
            }
        }
    }

    /// Marks what `section` holds from `offset` on.
    pub fn map(&mut self, section: SectionId, offset: u64, mapping: Mapping) -> SymbolId {
        self.symbols.push(Symbol {
            name: Vec::new(),
            section: Some(section),
            value: offset,
            size: 0,
            kind: SymbolKind::Mapping(mapping),
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
