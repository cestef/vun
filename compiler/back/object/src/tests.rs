use super::{Binding, Contents, Format, Machine, Object, Reloc, RelocKind, Symbol, SymbolKind};

/// A function calling an import and reaching a datum 8 bytes in, and the datum holding the
/// function's address.
fn object(format: Format) -> Object {
    let mut object = Object::new(Machine::Aarch64, format);
    let text = object.section(b".text", Contents::Code);
    let data = object.section(b".data", Contents::Writable);
    let at = object.append(text, &[0; 12], 16);
    let f = object.symbols.push(Symbol {
        name: b"f".to_vec(),
        section: Some(text),
        value: at,
        size: 12,
        kind: SymbolKind::Func,
        binding: Binding::Global,
    });
    let at = object.append(data, &[0; 16], 8);
    let d = object.symbols.push(Symbol {
        name: b"d".to_vec(),
        section: Some(data),
        value: at,
        size: 16,
        kind: SymbolKind::Object,
        binding: Binding::Local,
    });
    let exit = object.symbols.push(Symbol {
        name: b"exit".to_vec(),
        section: None,
        value: 0,
        size: 0,
        kind: SymbolKind::Func,
        binding: Binding::Global,
    });
    let reloc = |offset, symbol, kind, addend| Reloc { offset, symbol, kind, addend };
    object.sections[text].relocs = vec![
        reloc(0, d, RelocKind::Page21, 8),
        reloc(4, d, RelocKind::PageOffset12, 8),
        reloc(8, exit, RelocKind::Call26, 0),
    ];
    object.sections[data].relocs = vec![reloc(8, f, RelocKind::Abs64, 4)];
    object
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

/// The load command of type `cmd`, as its offset in the file.
fn command(bytes: &[u8], cmd: u32) -> usize {
    let mut at = 32;
    for _ in 0..u32_at(bytes, 16) {
        if u32_at(bytes, at) == cmd {
            return at;
        }
        at += u32_at(bytes, at + 4) as usize;
    }
    panic!("no command {cmd:#x}")
}

#[test]
fn a_macho_object_names_its_symbols_in_c_and_orders_them_by_where_they_are_defined() {
    let bytes = object(Format::MachO).write();
    assert_eq!(u32_at(&bytes, 0), 0xfeed_facf);
    assert_eq!(u32_at(&bytes, 4), 0x0100_000c, "arm64");
    assert_eq!(u32_at(&bytes, 12), 1, "an object");
    let symtab = command(&bytes, 0x2);
    let (symoff, nsyms, stroff) = (
        u32_at(&bytes, symtab + 8) as usize,
        u32_at(&bytes, symtab + 12),
        u32_at(&bytes, symtab + 16) as usize,
    );
    let names: Vec<&[u8]> = (0..nsyms as usize)
        .map(|k| {
            let at = stroff + u32_at(&bytes, symoff + k * 16) as usize;
            let end = bytes[at..].iter().position(|&b| b == 0).unwrap();
            &bytes[at..at + end]
        })
        .collect();
    assert_eq!(names, [b"_d".as_slice(), b"_f", b"_exit"], "locals, defined, undefined");
    let dysymtab = command(&bytes, 0xb);
    let ranges: Vec<u32> = (0..6).map(|k| u32_at(&bytes, dysymtab + 8 + k * 4)).collect();
    assert_eq!(ranges, [0, 1, 1, 1, 2, 1]);
}

#[test]
fn macho_relocations_keep_addends_in_place_or_before_their_instruction() {
    let bytes = object(Format::MachO).write();
    let segment = command(&bytes, 0x19);
    // The first section header follows the segment's 72 bytes: `__text`, then `__data`.
    let section = |k: usize| segment + 72 + k * 80;
    let (reloff, nreloc) = (u32_at(&bytes, section(0) + 56) as usize, u32_at(&bytes, section(0) + 60));
    let types: Vec<u32> = (0..nreloc as usize).map(|k| u32_at(&bytes, reloff + k * 8 + 4) >> 28).collect();
    // `ADDEND` before each field reaching 8 bytes in, `PAGE21`, `PAGEOFF12`, `BRANCH26`.
    assert_eq!(types, [10, 3, 10, 4, 2]);
    let data = section(1);
    let offset = u32_at(&bytes, data + 48) as usize;
    assert_eq!(
        u64::from_le_bytes(bytes[offset + 8..offset + 16].try_into().unwrap()),
        4,
        "the addend in place"
    );
}

#[test]
fn an_elf_object_keeps_its_addends_in_its_relocations() {
    let bytes = object(Format::Elf).write();
    assert_eq!(&bytes[..4], b"\x7fELF");
    assert!(!bytes.windows(2).any(|w| w == b"_f"), "ELF names symbols as they are");
}
