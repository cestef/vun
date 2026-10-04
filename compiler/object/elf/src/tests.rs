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
fn a_program_lists_the_bytes_each_loadable_segment_puts_where() {
    let bytes = program().write();
    let loads = File::loads(&bytes).unwrap();
    assert_eq!(loads.len(), 2);
    assert_eq!((loads[1].physical, loads[1].bytes), (0x40_2000, &[1, 2, 3][..]));
    assert!(matches!(File::loads(&bytes[..40]), Err(Malformed::Truncated { .. })));
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

/// `bytes` with the 64-bit word at `at` replaced by `value`.
fn patched(bytes: &[u8], at: usize, value: u64) -> Vec<u8> {
    let mut patched = bytes.to_vec();
    patched[at..at + 8].copy_from_slice(&value.to_le_bytes());
    patched
}

/// Where a 64-bit file's header keeps the offset of the program headers and of the section
/// headers, and the count of the sections; a program header's offset is its eighth byte, a section
/// header's its twenty-fourth and its size the thirty-second.
const PROGRAM_HEADERS: usize = 32;
const SECTION_HEADERS: usize = 40;
const SECTION_COUNT: usize = 60;
const PROGRAM_SIZE: usize = 56;
const SECTION_SIZE: usize = 64;

fn word_at(bytes: &[u8], at: usize) -> usize {
    usize::try_from(u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())).unwrap()
}

#[test]
fn a_program_without_section_headers_is_read_by_its_segments() {
    let mut bytes = program().write();
    bytes[SECTION_COUNT..SECTION_COUNT + 4].fill(0);
    let read = File::read(&bytes).unwrap();
    assert!(read.sections.is_empty());
    assert_eq!(read.segments.len(), 4);
}

#[test]
fn a_segment_reaching_past_the_address_space_is_refused() {
    let bytes = program().write();
    let second = word_at(&bytes, PROGRAM_HEADERS) + PROGRAM_SIZE;
    let huge = patched(&bytes, second + 8, u64::MAX - 1);
    assert_eq!(File::read(&huge), Err(Malformed::Range(1)));
    let wide = patched(&bytes, second + 16, u64::MAX - 1);
    assert_eq!(File::read(&wide), Err(Malformed::Range(1)));
}

#[test]
fn sections_holding_the_same_bytes_again_and_again_are_refused() {
    let mut file = object(Class::Elf64);
    file.sections[0] = Section::new(b".text", SectionKind::ProgBits, SectionFlags::ALLOC, vec![0x90; 600], 1);
    let bytes = file.write();
    let headers = word_at(&bytes, SECTION_HEADERS);
    let (text, rela) = (headers + SECTION_SIZE, headers + 2 * SECTION_SIZE);
    // The relocation section claims the text's bytes, and every byte up to the end of the file.
    let from = word_at(&bytes, text + 24);
    let claim =
        patched(&patched(&bytes, rela + 24, from as u64), rela + 32, bytes.len() as u64 - from as u64);
    assert_eq!(File::read(&claim), Err(Malformed::Overlap));
    assert!(File::read(&bytes).is_ok());
}

#[test]
fn a_name_longer_than_readers_copy_is_refused() {
    let mut file = object(Class::Elf64);
    file.sections[3] = Section::new(
        b".strtab",
        SectionKind::StrTab,
        SectionFlags::NONE,
        [&[0][..], &vec![b'a'; vun_bytes::NAME_MAX + 1], &[0]].concat(),
        1,
    );
    let read = File::read(&file.write()).unwrap();
    assert!(matches!(read.symbols(), Err(Malformed::Name(_))));
}
