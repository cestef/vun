use super::*;

/// A relocatable object of `class`: code relocated against a symbol, its symbol and string
/// tables, and the table of names.
fn object(class: Class) -> File {
    let symbols = [Symbol {
        name: b"f".to_vec(),
        kind: SymbolKind::Func,
        binding: Binding::Global,
        section: 1,
        value: 4,
        size: 2,
    }];
    let (table, strings) = Symbol::table(class, &symbols);
    let relocations = [Rela { offset: 8, symbol: 1, kind: 2, addend: -4 }];
    let mut rela = Section::new(
        b".rela.text",
        SectionKind::Rela,
        SectionFlags::INFO_LINK,
        Rela::table(class, &relocations),
        class.word(),
    );
    (rela.link, rela.info) = (3, 1);
    let mut symtab = Section::new(b".symtab", SectionKind::SymTab, SectionFlags::NONE, table, class.word());
    (symtab.link, symtab.info) = (4, 1);
    File {
        class,
        kind: Kind::Relocatable,
        machine: Machine::X86_64,
        flags: 0,
        entry: 0,
        sections: vec![
            Section::new(
                b".text",
                SectionKind::ProgBits,
                SectionFlags::ALLOC | SectionFlags::EXEC,
                vec![0x90; 12],
                16,
            ),
            rela,
            symtab,
            Section::new(b".strtab", SectionKind::StrTab, SectionFlags::NONE, strings, 1),
            Section::names(),
        ],
        segments: Vec::new(),
    }
}

#[test]
fn an_object_reads_back_as_it_was_written_in_either_class() {
    for class in [Class::Elf32, Class::Elf64] {
        let file = object(class);
        let read = File::read(&file.write()).unwrap();
        assert_eq!(read, file, "{class:?}");
        assert_eq!(read.symbol(b"f").unwrap().map(|s| s.value), Some(4));
        let rela = read.section(b".rela.text").unwrap();
        assert_eq!(Rela::read(class, rela).unwrap(), [Rela { offset: 8, symbol: 1, kind: 2, addend: -4 }]);
    }
}

/// A program: its headers and code loaded read and execute, its data and zeros read and write,
/// the program headers described, and a stack no segment makes executable.
fn program() -> File {
    let mut code = Section::new(
        b".text",
        SectionKind::ProgBits,
        SectionFlags::ALLOC | SectionFlags::EXEC,
        vec![0xc3; 5],
        16,
    );
    code.address = 0x40_1000;
    let mut data = Section::new(
        b".data",
        SectionKind::ProgBits,
        SectionFlags::ALLOC | SectionFlags::WRITE,
        vec![1, 2, 3],
        8,
    );
    data.address = 0x40_2000;
    let mut zeros =
        Section::new(b".bss", SectionKind::NoBits, SectionFlags::ALLOC | SectionFlags::WRITE, Vec::new(), 8);
    (zeros.address, zeros.contents) = (0x40_2008, Contents::Zeros(64));
    let segment = |kind, flags, covers| Segment { kind, flags, align: 0x1000, covers };
    File {
        class: Class::Elf64,
        kind: Kind::Executable,
        machine: Machine::X86_64,
        flags: 0,
        entry: 0x40_1000,
        sections: vec![code, data, zeros, Section::names()],
        segments: vec![
            segment(SegmentKind::Phdr, SegmentFlags::READ, Covers::ProgramHeaders),
            segment(
                SegmentKind::Load,
                SegmentFlags::READ | SegmentFlags::EXEC,
                Covers::Sections { headers: true, sections: 0..1 },
            ),
            segment(
                SegmentKind::Load,
                SegmentFlags::READ | SegmentFlags::WRITE,
                Covers::Sections { headers: false, sections: 1..3 },
            ),
            segment(SegmentKind::GnuStack, SegmentFlags::READ | SegmentFlags::WRITE, Covers::Nothing),
        ],
    }
}

#[test]
fn a_program_reads_back_with_its_segments_over_what_they_cover() {
    let file = program();
    let bytes = file.write();
    let read = File::read(&bytes).unwrap();
    assert_eq!(read, file);
    assert_eq!(read.loaded(0x40_2001), Some(&[2, 3][..]));
    assert_eq!(read.loaded(0x40_3000), None);
}

#[test]
fn bytes_that_are_not_little_endian_elf_are_refused() {
    let bytes = object(Class::Elf64).write();
    let mut foreign = bytes.clone();
    foreign[3] = b'G';
    assert_eq!(File::read(&foreign), Err(Malformed::Magic));
    let mut big = bytes.clone();
    big[5] = 2;
    assert_eq!(File::read(&big), Err(Malformed::BigEndian));
    let mut classless = bytes.clone();
    classless[4] = 3;
    assert_eq!(File::read(&classless), Err(Malformed::Class(3)));
    assert!(matches!(File::read(&bytes[..40]), Err(Malformed::Truncated { .. })));
}
