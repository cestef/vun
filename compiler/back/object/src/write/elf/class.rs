//! An ELF file's class: the width of its addresses and offsets, which its machine's pointers fix,
//! and the sizes and layouts of the entries that hold them.

use crate::Machine;

/// `ELFCLASS32` or `ELFCLASS64`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Class {
    Elf32,
    Elf64,
}

/// A symbol table entry's fields, laid out by `Class::symbol`.
pub(super) struct Entry {
    pub(super) name: u32,
    pub(super) info: u8,
    pub(super) section: u16,
    pub(super) value: u64,
    pub(super) size: u64,
}

impl Class {
    pub(super) const fn of(machine: Machine) -> Self {
        match machine.pointer_bits() {
            64 => Self::Elf64,
            _ => Self::Elf32,
        }
    }

    /// Its `EI_CLASS` number.
    pub(super) const fn ident(self) -> u8 {
        match self {
            Self::Elf32 => 1,
            Self::Elf64 => 2,
        }
    }

    /// The bytes of an address or offset, which tables of them are aligned to.
    pub(super) const fn word(self) -> u64 {
        match self {
            Self::Elf32 => 4,
            Self::Elf64 => 8,
        }
    }

    /// The size of the file header.
    pub(super) const fn header(self) -> u64 {
        match self {
            Self::Elf32 => 52,
            Self::Elf64 => 64,
        }
    }

    /// The size of a section header.
    pub(super) const fn section_header(self) -> u64 {
        match self {
            Self::Elf32 => 40,
            Self::Elf64 => 64,
        }
    }

    /// The size of a symbol table entry.
    pub(super) const fn symbol_size(self) -> u64 {
        match self {
            Self::Elf32 => 16,
            Self::Elf64 => 24,
        }
    }

    /// The size of a relocation with an addend: offset, info and addend, a word each.
    pub(super) const fn rela(self) -> u64 {
        3 * self.word()
    }

    /// Appends `value` as a word.
    ///
    /// # Panics
    /// Panics on a value past a 32-bit word.
    pub(super) fn put(self, out: &mut Vec<u8>, value: u64) {
        match self {
            Self::Elf32 => {
                out.extend_from_slice(&u32::try_from(value).expect("fits the class").to_le_bytes());
            }
            Self::Elf64 => out.extend_from_slice(&value.to_le_bytes()),
        }
    }

    /// Appends the signed `value` as a word.
    ///
    /// # Panics
    /// Panics on a value past a 32-bit word.
    pub(super) fn put_signed(self, out: &mut Vec<u8>, value: i64) {
        match self {
            Self::Elf32 => {
                out.extend_from_slice(&i32::try_from(value).expect("fits the class").to_le_bytes());
            }
            Self::Elf64 => out.extend_from_slice(&value.to_le_bytes()),
        }
    }

    /// A relocation's `r_info`: its symbol's number and its kind.
    pub(super) fn info(self, symbol: u32, kind: u32) -> u64 {
        match self {
            Self::Elf32 => u64::from(symbol) << 8 | u64::from(u8::try_from(kind).expect("kinds fit 8 bits")),
            Self::Elf64 => u64::from(symbol) << 32 | u64::from(kind),
        }
    }

    /// Appends a symbol table entry, its fields in the class's order.
    pub(super) fn symbol(self, out: &mut Vec<u8>, entry: &Entry) {
        out.extend_from_slice(&entry.name.to_le_bytes());
        let flags = |out: &mut Vec<u8>| {
            out.extend_from_slice(&[entry.info, 0]);
            out.extend_from_slice(&entry.section.to_le_bytes());
        };
        match self {
            Self::Elf32 => {
                self.put(out, entry.value);
                self.put(out, entry.size);
                flags(out);
            }
            Self::Elf64 => {
                flags(out);
                self.put(out, entry.value);
                self.put(out, entry.size);
            }
        }
    }
}
