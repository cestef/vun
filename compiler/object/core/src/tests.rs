use super::{
    Binding, Contents, Float, Format, Machine, Mapping, Object, Reloc, RelocKind, Symbol, SymbolKind,
};

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

/// Thumb code at `start`: `movw r0` and `movt r0` of `d`'s address, `bl f`, `beq.w f`, `b.w f`,
/// then `f: bx lr`, each field zero for its relocation; `d` holding `f`'s address and its
/// distance from there.
fn thumb() -> Object {
    let mut object = Object::new(Machine::Arm(Float::Hard), Format::Elf);
    let text = object.section(b".text", Contents::Code);
    let data = object.section(b".data", Contents::Writable);
    let code: [u16; 11] = [0xf240, 0, 0xf2c0, 0, 0xf000, 0xf800, 0xf000, 0x8000, 0xf000, 0xb800, 0x4770];
    let bytes: Vec<u8> = code.iter().flat_map(|h| h.to_le_bytes()).collect();
    let at = object.append(text, &bytes, 4);
    object.map(text, at, Mapping::Thumb);
    let symbol = |object: &mut Object, name: &[u8], section, value, kind| {
        object.symbols.push(Symbol {
            name: name.to_vec(),
            section,
            value,
            size: 0,
            kind,
            binding: Binding::Global,
        })
    };
    symbol(&mut object, b"_start", Some(text), at, SymbolKind::Func);
    let f = symbol(&mut object, b"f", Some(text), at + 20, SymbolKind::Func);
    let at = object.append(data, &[0; 8], 4);
    object.map(data, at, Mapping::Data);
    let d = symbol(&mut object, b"d", Some(data), at, SymbolKind::Object);
    // A branch reads the program counter 4 bytes past itself.
    let reloc = |offset, symbol, kind, addend| Reloc { offset, symbol, kind, addend };
    object.sections[text].relocs = vec![
        reloc(0, d, RelocKind::ThumbLow16, 0),
        reloc(4, d, RelocKind::ThumbHigh16, 0),
        reloc(8, f, RelocKind::ThumbCall, -4),
        reloc(12, f, RelocKind::ThumbJump19, -4),
        reloc(16, f, RelocKind::ThumbJump24, -4),
    ];
    object.sections[data].relocs =
        vec![reloc(0, f, Machine::Arm(Float::Hard).address(), 0), reloc(4, f, RelocKind::Pc32, 0)];
    object
}

#[test]
fn a_thumb_object_is_elf32_for_eabi5_with_hard_floats() {
    let bytes = thumb().write();
    assert_eq!(bytes[4], 1, "ELFCLASS32");
    assert_eq!(u16::from_le_bytes([bytes[18], bytes[19]]), 40, "EM_ARM");
    assert_eq!(u32_at(&bytes, 36), 0x0500_0400, "EABI 5, hard floats");
    assert_eq!(u16::from_le_bytes([bytes[40], bytes[41]]), 52, "its header");
    assert_eq!(u16::from_le_bytes([bytes[46], bytes[47]]), 40, "a section header");
}

/// The words of `program` at `symbol`'s address, as `llvm-readelf` and the file's program headers
/// place them: none where the tools are missing.
fn linked(name: &str, object: &Object) -> Option<Vec<u8>> {
    let dir = vun_fs::Scratch::new(name);
    let (input, output) = (dir.path().join("a.o"), dir.path().join("a"));
    std::fs::write(&input, object.write()).unwrap();
    let linked = std::process::Command::new("ld.lld").arg(&input).arg("-o").arg(&output).status().ok()?;
    assert!(linked.success(), "ld.lld links the object");
    Some(std::fs::read(output).unwrap())
}

/// The file offset of the loaded address `at` in the 32-bit ELF executable `file`.
fn offset_of(file: &[u8], at: u32) -> usize {
    let (phoff, phnum) = (u32_at(file, 28) as usize, u16::from_le_bytes([file[44], file[45]]));
    (0..usize::from(phnum))
        .map(|k| phoff + k * 32)
        .find_map(|p| {
            let (offset, vaddr, size) = (u32_at(file, p + 4), u32_at(file, p + 8), u32_at(file, p + 16));
            (u32_at(file, p) == 1 && (vaddr..vaddr + size).contains(&at))
                .then_some((offset + at - vaddr) as usize)
        })
        .expect("the address is loaded")
}

/// The value of the symbol `name` in the 32-bit ELF `file`.
fn value_of(file: &[u8], name: &[u8]) -> u32 {
    let (shoff, shnum) = (u32_at(file, 32) as usize, u16::from_le_bytes([file[48], file[49]]));
    let section = |k: usize| shoff + k * 40;
    let symtab =
        (0..usize::from(shnum)).map(section).find(|&s| u32_at(file, s + 4) == 2).expect("a symbol table");
    let strtab = section(u32_at(file, symtab + 24) as usize);
    let (symbols, strings) = (u32_at(file, symtab + 16) as usize, u32_at(file, strtab + 16) as usize);
    let count = u32_at(file, symtab + 20) as usize / 16;
    (0..count)
        .map(|k| symbols + k * 16)
        .find(|&s| {
            let at = strings + u32_at(file, s) as usize;
            file[at..].split(|&b| b == 0).next() == Some(name)
        })
        .map(|s| u32_at(file, s + 4))
        .expect("the symbol")
}

#[test]
fn ld_lld_resolves_thumb_relocations_with_their_addends_beside_them() {
    let Some(file) = linked("object-thumb", &thumb()) else { return };
    let (start, f, d) = (value_of(&file, b"_start"), value_of(&file, b"f"), value_of(&file, b"d"));
    assert_eq!((start & 1, f & 1), (1, 1), "Thumb functions have bit 0 set once linked");
    let halves = |at: u32, n: usize| -> Vec<u16> {
        let from = offset_of(&file, at);
        (0..n).map(|k| u16::from_le_bytes([file[from + 2 * k], file[from + 2 * k + 1]])).collect()
    };
    let code = halves(start & !1, 10);
    // `movw`/`movt`: imm16 = imm4:i:imm3:imm8 across the two halfwords.
    let imm16 =
        |hw1: u16, hw2: u16| (hw1 & 0xf) << 12 | (hw1 >> 10 & 1) << 11 | (hw2 >> 12 & 7) << 8 | hw2 & 0xff;
    let address = u32::from(imm16(code[2], code[3])) << 16 | u32::from(imm16(code[0], code[1]));
    assert_eq!(address, d);
    // `bl` and `b.w`: S:I1:I2:imm10:imm11:0, I = !(J ^ S); `b<c>.w`: S:J2:J1:imm6:imm11:0.
    let wide = |hw1: u16, hw2: u16| {
        let s = u32::from(hw1 >> 10 & 1);
        let (i1, i2) = (!(u32::from(hw2 >> 13 & 1) ^ s) & 1, !(u32::from(hw2 >> 11 & 1) ^ s) & 1);
        let raw = s << 24 | i1 << 23 | i2 << 22 | u32::from(hw1 & 0x3ff) << 12 | u32::from(hw2 & 0x7ff) << 1;
        (raw << 7).cast_signed() >> 7
    };
    let conditional = |hw1: u16, hw2: u16| {
        let raw = u32::from(hw1 >> 10 & 1) << 20
            | u32::from(hw2 >> 11 & 1) << 19
            | u32::from(hw2 >> 13 & 1) << 18
            | u32::from(hw1 & 0x3f) << 12
            | u32::from(hw2 & 0x7ff) << 1;
        (raw << 11).cast_signed() >> 11
    };
    let target = |at: u32, offset: i32| (at.cast_signed() + 4 + offset).cast_unsigned();
    let base = start & !1;
    assert_eq!(target(base + 8, wide(code[4], code[5])), f & !1, "bl");
    assert_eq!(target(base + 12, conditional(code[6], code[7])), f & !1, "beq.w");
    assert_eq!(target(base + 16, wide(code[8], code[9])), f & !1, "b.w");
    let words = offset_of(&file, d);
    assert_eq!(u32_at(&file, words), f, "the address of a Thumb function, bit 0 set");
    assert_eq!(u32_at(&file, words + 4), f.wrapping_sub(d + 4), "its distance from the field");
}
