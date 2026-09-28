//! The bytes of a Mach-O 64-bit relocatable object (`MH_OBJECT`): the header, one unnamed
//! segment holding every section, the platform, the symbol tables and their layout, then the
//! sections' bytes laid out as their addresses, their relocations, the symbols and their names.
//! Symbols are C names, which Mach-O spells with a leading underscore.

mod reloc;
mod symbols;

use crate::{Contents, Machine, Object, SectionId};

pub(super) const MAGIC: u32 = 0xfeed_facf;
/// `CPU_TYPE_*`, `CPU_SUBTYPE_*_ALL`.
pub(super) const ARM64: u32 = 0x0100_000c;
pub(super) const X86_64: u32 = 0x0100_0007;
pub(super) const ARM64_ALL: u32 = 0;
pub(super) const X86_64_ALL: u32 = 3;
/// `MH_OBJECT`, `MH_SUBSECTIONS_VIA_SYMBOLS`: each symbol starts a piece the linker may move.
pub(super) const OBJECT: u32 = 1;
pub(super) const SUBSECTIONS_VIA_SYMBOLS: u32 = 0x2000;
pub(super) const HEADER: u32 = 32;
/// `LC_*` and the sizes of their commands.
pub(super) const SEGMENT_64: u32 = 0x19;
pub(super) const SEGMENT: u32 = 72;
pub(super) const SECTION: u32 = 80;
pub(super) const BUILD_VERSION: u32 = 0x32;
pub(super) const BUILD: u32 = 24;
pub(super) const SYMTAB: u32 = 0x2;
pub(super) const SYMTAB_SIZE: u32 = 24;
pub(super) const DYSYMTAB: u32 = 0xb;
pub(super) const DYSYMTAB_SIZE: u32 = 80;
/// macOS 11, the first on Apple silicon.
pub(super) const PLATFORM_MACOS: u32 = 1;
pub(super) const MIN_OS: u32 = 0x000b_0000;
/// Read, write and execute.
pub(super) const PROT_ALL: u32 = 7;
/// `S_ATTR_*`.
pub(super) const PURE_INSTRUCTIONS: u32 = 0x8000_0000;
pub(super) const SOME_INSTRUCTIONS: u32 = 0x400;
pub(super) const DEBUG: u32 = 0x0200_0000;
/// `N_*`.
pub(super) const EXTERNAL: u8 = 0x1;
pub(super) const DEFINED: u8 = 0xe;
pub(super) const NLIST: u32 = 16;
pub(super) const RELOCATION: u32 = 8;
/// `ARM64_RELOC_*`.
pub(super) const UNSIGNED: u32 = 0;
pub(super) const BRANCH26: u32 = 2;
pub(super) const PAGE21: u32 = 3;
pub(super) const PAGEOFF12: u32 = 4;
pub(super) const GOT_LOAD_PAGE21: u32 = 5;
pub(super) const GOT_LOAD_PAGEOFF12: u32 = 6;
pub(super) const ADDEND: u32 = 10;
/// A relocation's length field: 4 or 8 bytes.
pub(super) const WORD32: u32 = 2;
pub(super) const WORD64: u32 = 3;

impl Contents {
    /// The segment and section a section of these contents is, and its flags; `None` for what
    /// Mach-O has no section for.
    fn macho(self) -> Option<(&'static [u8], &'static [u8], u32)> {
        Some(match self {
            Self::Code => (b"__TEXT", b"__text", PURE_INSTRUCTIONS | SOME_INSTRUCTIONS),
            Self::ReadOnly => (b"__TEXT", b"__const", 0),
            Self::Relocated => (b"__DATA", b"__const", 0),
            Self::Writable => (b"__DATA", b"__data", 0),
            Self::Debug => (b"__DWARF", b"", DEBUG),
            Self::StackNote => return None,
        })
    }
}

/// Little-endian words appended to a file.
pub(super) trait Put {
    fn put32(&mut self, v: u32);
    fn put64(&mut self, v: u64);
    /// A name padded to 16 bytes.
    fn name16(&mut self, name: &[u8]);
}

impl Put for Vec<u8> {
    fn put32(&mut self, v: u32) {
        self.extend_from_slice(&v.to_le_bytes());
    }

    fn put64(&mut self, v: u64) {
        self.extend_from_slice(&v.to_le_bytes());
    }

    fn name16(&mut self, name: &[u8]) {
        let mut field = [0; 16];
        let len = name.len().min(16);
        field[..len].copy_from_slice(&name[..len]);
        self.extend_from_slice(&field);
    }
}

/// A section as laid out: its address, which is also its offset past the commands.
pub(super) struct Placed {
    pub(super) id: SectionId,
    pub(super) addr: u64,
}

impl Object {
    /// The object as a Mach-O file.
    pub(super) fn macho(&self) -> Vec<u8> {
        let placed = self.place();
        let ordinal = |id: SectionId| placed.iter().position(|p| p.id == id).map(|k| k + 1);
        let order = self.macho_order();
        let count = u32::try_from(placed.len()).expect("sections fit 32 bits");
        let commands = SEGMENT + SECTION * count + BUILD + SYMTAB_SIZE + DYSYMTAB_SIZE;
        let start = u64::from(HEADER + commands);
        let end = placed.last().map_or(0, |p| p.addr + self.sections[p.id].bytes.len() as u64);
        // The sections' bytes, with the addends of relocations Mach-O keeps in place.
        let mut data = vec![0; usize::try_from(end).expect("objects fit memory")];
        let mut relocs: Vec<Vec<u8>> = Vec::new();
        for p in &placed {
            let s = &self.sections[p.id];
            let at = usize::try_from(p.addr).expect("objects fit memory");
            data[at..at + s.bytes.len()].copy_from_slice(&s.bytes);
            let mut written = Vec::new();
            for r in &s.relocs {
                self.relocate(r, &placed, &order, &mut data[at..], &mut written);
            }
            relocs.push(written);
        }
        let mut out = Vec::new();
        out.put32(MAGIC);
        let (cpu, subtype) = match self.machine {
            Machine::Aarch64 => (ARM64, ARM64_ALL),
            Machine::X86_64 => (X86_64, X86_64_ALL),
            Machine::Arm(_) => panic!("Mach-O objects are written for 64-bit machines"),
        };
        for word in [cpu, subtype, OBJECT, 4, commands, SUBSECTIONS_VIA_SYMBOLS, 0] {
            out.put32(word);
        }
        let mut reloff = start + end;
        let symoff = (reloff + relocs.iter().map(|r| r.len() as u64).sum::<u64>()).next_multiple_of(8);
        out.put32(SEGMENT_64);
        out.put32(SEGMENT + SECTION * count);
        out.name16(b"");
        for word in [0, end, start, end] {
            out.put64(word);
        }
        for word in [PROT_ALL, PROT_ALL, count, 0] {
            out.put32(word);
        }
        for (p, written) in placed.iter().zip(&relocs) {
            let s = &self.sections[p.id];
            let (segment, name, flags) = s.contents.macho().expect("placed sections have one");
            // A debugging section keeps its DWARF name: `.debug_line` is `__debug_line`.
            let name = if name.is_empty() { [b"__", &s.name[1..]].concat() } else { name.to_vec() };
            out.name16(&name);
            out.name16(segment);
            out.put64(p.addr);
            out.put64(s.bytes.len() as u64);
            let nreloc = u32::try_from(written.len() as u64 / u64::from(RELOCATION)).expect("fits");
            let off = if nreloc == 0 { 0 } else { reloff };
            reloff += written.len() as u64;
            for word in [
                u32::try_from(start + p.addr).expect("objects fit 4 GiB"),
                s.align.max(1).trailing_zeros(),
                u32::try_from(off).expect("objects fit 4 GiB"),
                nreloc,
                flags,
                0,
                0,
                0,
            ] {
                out.put32(word);
            }
        }
        for word in [BUILD_VERSION, BUILD, PLATFORM_MACOS, MIN_OS, 0, 0] {
            out.put32(word);
        }
        let (symbols, strings) = self.macho_symbols(&order, &placed, ordinal);
        let nsyms = u32::try_from(order.symbols.len()).expect("symbols fit 32 bits");
        let stroff = symoff + u64::from(nsyms * NLIST);
        for word in [SYMTAB, SYMTAB_SIZE, u32::try_from(symoff).expect("fits"), nsyms] {
            out.put32(word);
        }
        out.put32(u32::try_from(stroff).expect("objects fit 4 GiB"));
        out.put32(u32::try_from(strings.len()).expect("fits"));
        out.put32(DYSYMTAB);
        out.put32(DYSYMTAB_SIZE);
        for word in [0, order.locals, order.locals, order.defined, order.locals + order.defined] {
            out.put32(word);
        }
        out.put32(nsyms - order.locals - order.defined);
        out.extend_from_slice(&[0; 48]);
        out.extend_from_slice(&data);
        for written in &relocs {
            out.extend_from_slice(written);
        }
        out.resize(usize::try_from(symoff).expect("fits"), 0);
        out.extend_from_slice(&symbols);
        out.extend_from_slice(&strings);
        out
    }

    /// The sections Mach-O has, at addresses one after the other, each at its alignment.
    fn place(&self) -> Vec<Placed> {
        let mut addr = 0u64;
        let written = self.sections.iter().filter(|(_, s)| s.contents.macho().is_some());
        written
            .map(|(id, s)| {
                addr = addr.next_multiple_of(s.align.max(1));
                let placed = Placed { id, addr };
                addr += s.bytes.len() as u64;
                placed
            })
            .collect()
    }
}
