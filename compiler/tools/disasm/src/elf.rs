//! The code of an ELF file: its executable sections, and the symbols of its symbol table.

use vun_elf::{File, Kind, Machine, SectionFlags, SymbolKind};

use crate::Isa;
use crate::code::{Code, Error, Part, Symbol};

/// The first section number that is no section: the numbers from it on are special (absolute
/// symbols, common ones).
const SPECIAL: u16 = 0xff00;

impl Code {
    /// The code of `file`.
    ///
    /// # Errors
    /// Where it is for a machine whose instructions are not read, or its symbols are malformed.
    pub fn of_elf(file: &File) -> Result<Self, Error> {
        let isa = Isa::of_elf(file.machine).ok_or_else(|| Error::Machine(format!("{:?}", file.machine)))?;
        // Section `k` of the file is `sections[k - 1]`; a part is a section of code.
        let mut part_of = vec![None; file.sections.len() + 1];
        let mut parts = Vec::new();
        for (number, section) in (1..).zip(&file.sections) {
            if section.flags.contains(SectionFlags::EXEC) {
                part_of[number] = Some(parts.len());
                parts.push(Part::loaded(&section.name, section.address, section.bytes().to_vec())?);
            }
        }
        let symbols = file.symbols().map_err(Error::Elf)?;
        let symbols = symbols
            .into_iter()
            .filter(|s| !s.name.is_empty() && !matches!(s.kind, SymbolKind::Section | SymbolKind::File))
            .filter(|s| (1..SPECIAL).contains(&s.section))
            .map(|s| Symbol {
                part: part_of.get(usize::from(s.section)).copied().flatten(),
                name: s.name,
                address: s.value,
                size: s.size,
            })
            .collect();
        Ok(Self { isa, parts, symbols, linked: file.kind != Kind::Relocatable })
    }
}

impl Isa {
    /// The instruction set of the code of an ELF file for `machine`, if one can be read.
    #[must_use]
    pub fn of_elf(machine: Machine) -> Option<Self> {
        match machine {
            Machine::X86_64 => Some(Self::X86_64),
            Machine::Aarch64 => Some(Self::Arm64),
            Machine::Other(EM_386) => Some(Self::X86),
            _ => None,
        }
    }
}

/// `EM_386`: 32-bit x86, which the file's machines do not name.
const EM_386: u16 = 3;
