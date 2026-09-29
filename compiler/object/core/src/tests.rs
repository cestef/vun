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
    let file = vun_elf::File::read(&object(Format::Elf).write()).unwrap();
    let symbols = file.symbols().unwrap();
    assert!(symbols.iter().any(|s| s.name == b"f"), "ELF names symbols as they are");
    let relocations = vun_elf::Rela::read(file.class, file.section(b".rela.data").unwrap()).unwrap();
    let [address] = relocations.as_slice() else { panic!("one relocation of the datum") };
    assert_eq!((address.offset, address.addend), (8, 4));
    let named = &symbols[usize::try_from(address.symbol - 1).unwrap()];
    assert_eq!(named.name, b"f");
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
    let file = vun_elf::File::read(&thumb().write()).unwrap();
    assert_eq!((file.class, file.machine), (vun_elf::Class::Elf32, vun_elf::Machine::Arm));
    assert_eq!(file.flags, vun_elf::arm::EABI5 | vun_elf::arm::FloatAbi::Hard.number());
    assert!(file.section(b".ARM.attributes").is_some());
}

/// The program `ld.lld` links of `object`: none where it is missing.
fn linked(name: &str, object: &Object) -> Option<vun_elf::File> {
    let dir = vun_fs::Scratch::new(name);
    let (input, output) = (dir.path().join("a.o"), dir.path().join("a"));
    std::fs::write(&input, object.write()).unwrap();
    let linked = std::process::Command::new("ld.lld").arg(&input).arg("-o").arg(&output).status().ok()?;
    assert!(linked.success(), "ld.lld links the object");
    Some(vun_elf::File::read(&std::fs::read(output).unwrap()).unwrap())
}

/// The value of the symbol `name` in `file`, a 32-bit program.
fn value_of(file: &vun_elf::File, name: &[u8]) -> u32 {
    u32::try_from(file.symbol(name).unwrap().expect("the symbol").value).unwrap()
}

/// The 32-bit word loaded at `at` in `file`.
fn word_at(file: &vun_elf::File, at: u32) -> u32 {
    u32_at(file.loaded(u64::from(at)).expect("the address is loaded"), 0)
}

#[test]
fn ld_lld_resolves_thumb_relocations_with_their_addends_beside_them() {
    let Some(file) = linked("object-thumb", &thumb()) else { return };
    let (start, f, d) = (value_of(&file, b"_start"), value_of(&file, b"f"), value_of(&file, b"d"));
    assert_eq!((start & 1, f & 1), (1, 1), "Thumb functions have bit 0 set once linked");
    let halves = |at: u32, n: usize| -> Vec<u16> {
        let bytes = file.loaded(u64::from(at)).expect("the code is loaded");
        (0..n).map(|k| u16::from_le_bytes([bytes[2 * k], bytes[2 * k + 1]])).collect()
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
    assert_eq!(word_at(&file, d), f, "the address of a Thumb function, bit 0 set");
    assert_eq!(word_at(&file, d + 4), f.wrapping_sub(d + 4), "its distance from the field");
}
