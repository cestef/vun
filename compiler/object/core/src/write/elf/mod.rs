//! The bytes of an ELF relocatable object of the machine's class: the header, each section's
//! bytes, their relocations, the symbol and string tables, then the section headers.

mod class;
mod numbers;

use vun_index::{Idx, IndexVec};

use super::Strings;
use crate::{Binding, Contents, Object, SectionId, SymbolId, SymbolKind};
use class::{Class, Entry};

/// `SHT_*`
const PROGBITS: u32 = 1;
const SYMTAB: u32 = 2;
const STRTAB: u32 = 3;
const RELA_TYPE: u32 = 4;
/// `SHF_*`
const WRITE: u64 = 1;
const ALLOC: u64 = 2;
const EXEC: u64 = 4;
const INFO_LINK: u64 = 0x40;
/// `ET_REL`, `EV_CURRENT`, `ELFDATA2LSB`.
const RELOCATABLE: u16 = 1;
const CURRENT: u8 = 1;
const LITTLE: u8 = 1;
const MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];

/// A section header, as written.
#[derive(Default)]
struct Header {
    name: u32,
    kind: u32,
    flags: u64,
    offset: u64,
    size: u64,
    link: u32,
    info: u32,
    align: u64,
    entsize: u64,
}

impl Contents {
    const fn flags(self) -> u64 {
        match self {
            Self::Code => ALLOC | EXEC,
            Self::ReadOnly | Self::Relocated => ALLOC,
            Self::Writable => ALLOC | WRITE,
            Self::StackNote | Self::Debug => 0,
        }
    }
}

/// The file under construction: its bytes, the headers of the sections written, and their
/// names.
struct Writer {
    class: Class,
    out: Vec<u8>,
    headers: Vec<Header>,
    names: Strings,
}

impl Writer {
    /// Pads the file to `align`: where the next part starts.
    fn at(&mut self, align: u64) -> u64 {
        let len = (self.out.len() as u64).next_multiple_of(align.max(1));
        self.out.resize(usize::try_from(len).expect("files fit memory"), 0);
        len
    }

    fn header(&mut self, name: &[u8], mut header: Header) {
        header.name = self.names.add(name);
        self.headers.push(header);
    }

    /// The number of the section header of `id`.
    fn number(id: SectionId) -> u32 {
        u32::try_from(id.index() + 1).expect("sections fit 32 bits")
    }
}

impl Object {
    /// The symbols in the order of the table, locals first as the format asks, and each one's
    /// number there.
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

    fn write_sections(&self, w: &mut Writer) {
        for (_, s) in self.sections.iter() {
            let offset = w.at(s.align);
            let header = Header {
                kind: PROGBITS,
                flags: s.contents.flags(),
                offset,
                size: s.bytes.len() as u64,
                align: s.align.max(1),
                ..Header::default()
            };
            w.header(&s.name, header);
            w.out.extend_from_slice(&s.bytes);
        }
    }

    /// The relocations of each section having some, which name symbols by `number`, the table
    /// being section `symtab`.
    fn write_relocations(&self, w: &mut Writer, number: &IndexVec<SymbolId, u32>, symtab: u32) {
        let class = w.class;
        for (id, s) in self.sections.iter().filter(|(_, s)| !s.relocs.is_empty()) {
            let offset = w.at(class.word());
            for r in &s.relocs {
                let kind = self
                    .machine
                    .elf_relocation(r.kind)
                    .expect("backends emit the relocations their machine has");
                class.put(&mut w.out, r.offset);
                class.put(&mut w.out, class.info(number[r.symbol], kind));
                class.put_signed(&mut w.out, r.addend);
            }
            let header = Header {
                kind: RELA_TYPE,
                flags: INFO_LINK,
                offset,
                size: s.relocs.len() as u64 * class.rela(),
                link: symtab,
                info: Writer::number(id),
                align: class.word(),
                entsize: class.rela(),
                ..Header::default()
            };
            w.header(&[b".rela".as_slice(), &s.name].concat(), header);
        }
    }

    /// The symbol table, then its strings.
    fn write_symbols(&self, w: &mut Writer, order: &[SymbolId], symtab: u32) {
        let class = w.class;
        let mut strings = Strings::default();
        let offset = w.at(class.word());
        w.out.resize(w.out.len() + usize::try_from(class.symbol_size()).expect("fits"), 0);
        for &id in order {
            let s = &self.symbols[id];
            let name = match s.kind {
                SymbolKind::Mapping(mapping) => mapping.elf(),
                _ => &s.name,
            };
            let entry = Entry {
                name: strings.add(name),
                info: s.binding.elf() << 4 | s.kind.elf(),
                section: s
                    .section
                    .map_or(0, |s| u16::try_from(Writer::number(s)).expect("sections fit 16 bits")),
                value: if s.section.is_some() { self.machine.elf_value(s.kind, s.value) } else { s.value },
                size: s.size,
            };
            class.symbol(&mut w.out, &entry);
        }
        let locals = order.iter().take_while(|&&id| self.symbols[id].binding == Binding::Local).count();
        let header = Header {
            kind: SYMTAB,
            offset,
            size: (order.len() as u64 + 1) * class.symbol_size(),
            link: symtab + 1,
            info: u32::try_from(locals + 1).expect("symbols fit 32 bits"),
            align: class.word(),
            entsize: class.symbol_size(),
            ..Header::default()
        };
        w.header(b".symtab", header);
        let strings = if strings.0.is_empty() { vec![0] } else { strings.0 };
        let header = Header {
            kind: STRTAB,
            offset: w.out.len() as u64,
            size: strings.len() as u64,
            align: 1,
            ..Header::default()
        };
        w.header(b".strtab", header);
        w.out.extend_from_slice(&strings);
    }

    /// The object as an ELF file.
    pub(super) fn elf(&self) -> Vec<u8> {
        let class = Class::of(self.machine);
        let mut w = Writer {
            class,
            out: vec![0; usize::try_from(class.header()).expect("fits")],
            headers: vec![Header::default()],
            names: Strings::default(),
        };
        let (order, number) = self.symbol_order();
        self.write_sections(&mut w);
        let relocated = self.sections.iter().filter(|(_, s)| !s.relocs.is_empty()).count();
        let symtab = u32::try_from(w.headers.len() + relocated).expect("sections fit 32 bits");
        self.write_relocations(&mut w, &number, symtab);
        self.write_symbols(&mut w, &order, symtab);
        let shstrtab = u16::try_from(w.headers.len()).expect("sections fit 16 bits");
        let name = w.names.add(b".shstrtab");
        let table = std::mem::take(&mut w.names.0);
        let offset = w.out.len() as u64;
        w.headers.push(Header {
            name,
            kind: STRTAB,
            offset,
            size: table.len() as u64,
            align: 1,
            ..Header::default()
        });
        w.out.extend_from_slice(&table);
        let shoff = w.at(class.word());
        let mut headers = Vec::new();
        for h in &w.headers {
            for word in [h.name, h.kind] {
                headers.extend_from_slice(&word.to_le_bytes());
            }
            for word in [h.flags, 0, h.offset, h.size] {
                class.put(&mut headers, word);
            }
            for word in [h.link, h.info] {
                headers.extend_from_slice(&word.to_le_bytes());
            }
            for word in [h.align, h.entsize] {
                class.put(&mut headers, word);
            }
        }
        w.out.append(&mut headers);
        let shnum = u16::try_from(w.headers.len()).expect("sections fit 16 bits");
        let header = self.file_header(class, shoff, shnum, shstrtab);
        w.out[..header.len()].copy_from_slice(&header);
        w.out
    }

    /// The ELF header of `class`, its section headers at `shoff`.
    fn file_header(&self, class: Class, shoff: u64, shnum: u16, shstrtab: u16) -> Vec<u8> {
        let mut header = Vec::new();
        header.extend_from_slice(&MAGIC);
        header.extend_from_slice(&[class.ident(), LITTLE, CURRENT, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        header.extend_from_slice(&RELOCATABLE.to_le_bytes());
        header.extend_from_slice(&self.machine.elf().to_le_bytes());
        header.extend_from_slice(&u32::from(CURRENT).to_le_bytes());
        for word in [0, 0, shoff] {
            class.put(&mut header, word);
        }
        header.extend_from_slice(&self.machine.elf_flags().to_le_bytes());
        let size = |bytes: u64| u16::try_from(bytes).expect("headers are small");
        for half in [size(class.header()), 0, 0, size(class.section_header()), shnum, shstrtab] {
            header.extend_from_slice(&half.to_le_bytes());
        }
        header
    }
}
