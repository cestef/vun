//! The records of the file as ELF lays them out: the file header, a section header, a program
//! header, a symbol table entry and a relocation with its addend.

use crate::record::{SignedWord, Word};

record! {
    /// `Elf_Ehdr`.
    struct FileHeader {
        ident: [u8; 16],
        kind: u16,
        machine: u16,
        version: u32,
        entry: Word,
        program_headers: Word,
        section_headers: Word,
        flags: u32,
        header_size: u16,
        program_header_size: u16,
        program_header_count: u16,
        section_header_size: u16,
        section_header_count: u16,
        names: u16,
    }
}

record! {
    /// `Elf_Shdr`.
    struct SectionHeader {
        name: u32,
        kind: u32,
        flags: Word,
        address: Word,
        offset: Word,
        size: Word,
        link: u32,
        info: u32,
        align: Word,
        entsize: Word,
    }
}

record! {
    /// `Elf_Phdr`: its flags come second in `ELFCLASS64`, to align the words after them.
    struct ProgramHeader {
        kind: u32,
        flags: u32,
        offset: Word,
        address: Word,
        physical: Word,
        file_size: Word,
        memory_size: Word,
        align: Word,
    }
    elf32 [kind, offset, address, physical, file_size, memory_size, flags, align]
    elf64 [kind, flags, offset, address, physical, file_size, memory_size, align]
}

record! {
    /// `Elf_Sym`: its value and size come first in `ELFCLASS32`, last in `ELFCLASS64`.
    struct SymbolEntry {
        name: u32,
        info: u8,
        other: u8,
        section: u16,
        value: Word,
        size: Word,
    }
    elf32 [name, value, size, info, other, section]
    elf64 [name, info, other, section, value, size]
}

record! {
    /// `Elf_Rela`.
    struct RelaEntry {
        offset: Word,
        info: Word,
        addend: SignedWord,
    }
}
