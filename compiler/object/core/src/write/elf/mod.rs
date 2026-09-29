//! An object as an ELF relocatable file of its machine's class (`vun_elf`): each section's bytes,
//! their relocations, the symbol and string tables, the build attributes of 32-bit Arm, and the
//! table of names.

mod attributes;
mod numbers;

use vun_elf::{Class, File, Kind, Rela, Section, SectionFlags, SectionKind};
use vun_index::{Idx, IndexVec};

use crate::{Binding, Contents, Object, SectionId, SymbolId, SymbolKind};

impl Contents {
    fn flags(self) -> SectionFlags {
        match self {
            Self::Code => SectionFlags::ALLOC | SectionFlags::EXEC,
            Self::ReadOnly | Self::Relocated => SectionFlags::ALLOC,
            Self::Writable | Self::ThreadVariables => SectionFlags::ALLOC | SectionFlags::WRITE,
            Self::PerThread => SectionFlags::ALLOC | SectionFlags::WRITE | SectionFlags::TLS,
            Self::StackNote | Self::Debug => SectionFlags::NONE,
        }
    }
}

impl Object {
    /// The object as an ELF file's bytes.
    pub(super) fn elf(&self) -> Vec<u8> {
        self.elf_file().write()
    }

    /// The object as an ELF file.
    fn elf_file(&self) -> File {
        let class = Class::of_bits(self.machine.pointer_bits());
        let (order, number) = self.symbol_order();
        let mut sections: Vec<Section> = self
            .sections
            .iter()
            .map(|(_, s)| {
                Section::new(
                    &s.name,
                    SectionKind::ProgBits,
                    s.contents.flags(),
                    s.bytes.clone(),
                    s.align.max(1),
                )
            })
            .collect();
        let relocated = self.sections.iter().filter(|(_, s)| !s.relocs.is_empty()).count();
        let symtab = File::number(sections.len() + relocated);
        for (id, s) in self.sections.iter().filter(|(_, s)| !s.relocs.is_empty()) {
            let relocations: Vec<Rela> = s
                .relocs
                .iter()
                .map(|r| Rela {
                    offset: r.offset,
                    symbol: number[r.symbol],
                    kind: self
                        .machine
                        .elf_relocation(r.kind)
                        .expect("backends emit the relocations their machine has"),
                    addend: r.addend,
                })
                .collect();
            let name = [b".rela".as_slice(), &s.name].concat();
            let mut rela = Section::new(
                &name,
                SectionKind::Rela,
                SectionFlags::INFO_LINK,
                Rela::table(class, &relocations),
                class.word(),
            );
            (rela.link, rela.info, rela.entsize) = (symtab, File::number(id.index()), Rela::size(class));
            sections.push(rela);
        }
        let (table, strings) = vun_elf::Symbol::table(class, &self.elf_symbols(&order));
        let locals = order.iter().take_while(|&&id| self.symbols[id].binding == Binding::Local).count();
        let mut symbols =
            Section::new(b".symtab", SectionKind::SymTab, SectionFlags::NONE, table, class.word());
        (symbols.link, symbols.info, symbols.entsize) =
            (symtab + 1, File::number(locals), vun_elf::Symbol::size(class));
        sections.push(symbols);
        sections.push(Section::new(b".strtab", SectionKind::StrTab, SectionFlags::NONE, strings, 1));
        if let Some(attributes) = self.machine.elf_attributes() {
            sections.push(Section::new(
                b".ARM.attributes",
                SectionKind::ArmAttributes,
                SectionFlags::NONE,
                attributes,
                1,
            ));
        }
        sections.push(Section::names());
        File {
            class,
            kind: Kind::Relocatable,
            machine: self.machine.elf(),
            flags: self.machine.elf_flags(),
            entry: 0,
            sections,
            segments: Vec::new(),
        }
    }

    /// The symbols in the order of the table, locals first as the format asks, and each one's
    /// number there, after the null entry.
    fn symbol_order(&self) -> (Vec<SymbolId>, IndexVec<SymbolId, u32>) {
        let order: Vec<SymbolId> = [Binding::Local, Binding::Global]
            .into_iter()
            .flat_map(|b| self.symbols.iter().filter(move |(_, s)| s.binding == b).map(|(id, _)| id))
            .collect();
        let mut number: IndexVec<SymbolId, u32> = IndexVec::filled(0, self.symbols.len());
        for (i, &id) in order.iter().enumerate() {
            number[id] = u32::try_from(i + 1).expect("symbols fit 32 bits");
        }
        (order, number)
    }

    /// The symbols `order` lists, as the file's symbol table holds them.
    fn elf_symbols(&self, order: &[SymbolId]) -> Vec<vun_elf::Symbol> {
        let section = |id: SectionId| u16::try_from(File::number(id.index())).expect("sections fit 16 bits");
        order
            .iter()
            .map(|&id| {
                let s = &self.symbols[id];
                vun_elf::Symbol {
                    name: match s.kind {
                        SymbolKind::Mapping(mapping) => mapping.elf().to_vec(),
                        _ => s.name.clone(),
                    },
                    kind: s.kind.elf(),
                    binding: s.binding.elf(),
                    section: s.section.map_or(0, section),
                    value: if s.section.is_some() { self.machine.reached(s.kind, s.value) } else { s.value },
                    size: s.size,
                }
            })
            .collect()
    }
}
