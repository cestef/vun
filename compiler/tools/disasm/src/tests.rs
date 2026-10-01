use vun_elf::{Binding, Class, File, Kind, Machine, Section, SectionFlags, SectionKind};

use super::*;

/// An object whose code is `code`, with symbols `(name, offset, size)` placed in it.
fn object(machine: Machine, code: &[u8], symbols: &[(&[u8], u64, u64)]) -> File {
    let symbols: Vec<vun_elf::Symbol> = symbols
        .iter()
        .map(|&(name, value, size)| vun_elf::Symbol {
            name: name.to_vec(),
            kind: vun_elf::SymbolKind::Func,
            binding: Binding::Global,
            section: 1,
            value,
            size,
        })
        .collect();
    let (table, strings) = vun_elf::Symbol::table(Class::Elf64, &symbols);
    let mut symtab = Section::new(b".symtab", SectionKind::SymTab, SectionFlags::NONE, table, 8);
    (symtab.link, symtab.info) = (3, 1);
    File {
        class: Class::Elf64,
        kind: Kind::Relocatable,
        machine,
        flags: 0,
        entry: 0,
        sections: vec![
            Section::new(
                b".text",
                SectionKind::ProgBits,
                SectionFlags::ALLOC | SectionFlags::EXEC,
                code.to_vec(),
                16,
            ),
            symtab,
            Section::new(b".strtab", SectionKind::StrTab, SectionFlags::NONE, strings, 1),
            Section::names(),
        ],
        segments: Vec::new(),
    }
}

/// The listing of the code of `file`.
fn list(file: &File, only: Option<&[u8]>) -> Result<Vec<Line>, Error> {
    listing(&Code::of_elf(file)?, only)
}

fn shown(lines: &[Line]) -> Vec<String> {
    lines.iter().map(ToString::to_string).collect()
}

/// `push rbp; mov rbp, rsp; pop rbp; ret`.
const X86: [u8; 6] = [0x55, 0x48, 0x89, 0xe5, 0x5d, 0xc3];

#[test]
fn x86_64_code_is_listed_instruction_by_instruction_under_its_labels() {
    let file = object(Machine::X86_64, &X86, &[(b"f", 0, 6)]);
    let listed = shown(&list(&file, None).unwrap());
    assert_eq!(listed[0], "section .text at 0x0");
    assert_eq!(listed[1], "0000000000000000 <f>:");
    assert!(listed[2].ends_with("push rbp"), "{}", listed[2]);
    assert!(listed[3].ends_with("mov rbp, rsp"), "{}", listed[3]);
    assert!(listed[4].ends_with("pop rbp"), "{}", listed[4]);
    assert!(listed[5].ends_with("ret"), "{}", listed[5]);
    assert_eq!(listed.len(), 6);
}

#[test]
fn arm64_code_is_listed_a_word_at_a_time() {
    // `ret`, then a word that is no instruction.
    let code = [0xc0, 0x03, 0x5f, 0xd6, 0xff, 0xff, 0xff, 0xff];
    let file = object(Machine::Aarch64, &code, &[]);
    let listed = shown(&list(&file, None).unwrap());
    assert!(listed[1].ends_with("ret"), "{}", listed[1]);
    assert!(listed[2].ends_with(".byte 0xff"), "{}", listed[2]);
}

#[test]
fn bytes_that_start_no_instruction_are_listed_as_data_one_at_a_time() {
    let lines = list(&object(Machine::X86_64, &[0x06, 0xc3], &[]), None).unwrap();
    assert_eq!(
        lines[1..],
        [
            Line::Data { address: 0, byte: 0x06 },
            Line::Inst { address: 1, bytes: vec![0xc3], text: "ret".to_owned(), target: None },
        ]
    );
}

#[test]
fn one_symbol_lists_the_code_it_labels() {
    let file = object(Machine::X86_64, &X86, &[(b"f", 0, 1), (b"g", 1, 0)]);
    let lines = list(&file, Some(b"g")).unwrap();
    let texts: Vec<_> = lines
        .iter()
        .filter_map(|l| if let Line::Inst { text, .. } = l { Some(text.as_str()) } else { None })
        .collect();
    assert_eq!(texts, ["mov rbp, rsp", "pop rbp", "ret"]);
    assert_eq!(list(&file, Some(b"f")).unwrap().len(), 3);
}

#[test]
fn a_name_that_labels_no_code_is_refused() {
    let file = object(Machine::X86_64, &X86, &[(b"f", 0, 6)]);
    assert_eq!(list(&file, Some(b"h")), Err(Error::Missing(b"h".to_vec())));
}

#[test]
fn a_machine_whose_instructions_are_not_read_is_refused() {
    let file = object(Machine::Arm, &[0; 4], &[]);
    assert_eq!(list(&file, None), Err(Error::Machine("Arm".to_owned())));
}

/// A program: `call f` (5 bytes at 0x1000), `jmp` over a byte, `nop`, then `f` at 0x1008.
fn program() -> File {
    let code = [0xe8, 0x03, 0x00, 0x00, 0x00, 0xeb, 0x01, 0x90, 0xc3];
    let mut file = object(Machine::X86_64, &code, &[(b"main", 0x1000, 8), (b"f", 0x1008, 1)]);
    file.kind = Kind::Executable;
    file.sections[0].address = 0x1000;
    file
}

#[test]
fn a_direct_branch_goes_to_the_symbol_that_holds_its_target() {
    let lines = list(&program(), None).unwrap();
    let targets: Vec<_> = lines
        .iter()
        .filter_map(|l| if let Line::Inst { target: Some(t), .. } = l { Some(t.to_string()) } else { None })
        .collect();
    assert_eq!(targets, ["0x1008 <f>", "0x1008 <f>"]);
    let text = shown(&lines[2..3]);
    assert!(text[0].ends_with("call $+0x3  ; 0x1008 <f>"), "{}", text[0]);
}

#[test]
fn a_target_inside_a_symbol_is_named_by_its_distance() {
    let mut file = program();
    // `jmp` over the `ret` into the middle of `main`: a distance of -0x5 from its end.
    file.sections[0].contents = vun_elf::Contents::Bytes(vec![0x90, 0x90, 0xeb, 0xfe]);
    let lines = list(&file, Some(b"main")).unwrap();
    let Some(Line::Inst { target: Some(target), .. }) = lines.last() else { panic!("{lines:?}") };
    assert_eq!(target.to_string(), "0x1002 <main+0x2>");
}

#[test]
fn an_object_names_no_target() {
    let code = [0xe8, 0x00, 0x00, 0x00, 0x00];
    let lines = list(&object(Machine::X86_64, &code, &[(b"f", 0, 5)]), None).unwrap();
    assert!(matches!(&lines[2], Line::Inst { target: Some(Target { address: 5, symbol: None }), .. }));
}

#[test]
fn an_instruction_ends_where_the_next_label_begins() {
    // A stray zero byte before `push rbp; ret` at the label `f`: `00 55 48` would be one
    // instruction, swallowing the start of `f`.
    let file = object(Machine::X86_64, &[0x00, 0x55, 0xc3], &[(b"f", 1, 2)]);
    let lines = list(&file, None).unwrap();
    assert_eq!(lines[1], Line::Data { address: 0, byte: 0 });
    assert_eq!(lines[2], Line::Label { address: 1, name: b"f".to_vec() });
    assert!(matches!(&lines[3], Line::Inst { text, .. } if text == "push rbp"));
}

/// A 64-bit program of one section of code at 0x1000, named by its entry and an export at `helper`, and
/// importing `WriteFile` through a slot at 0x2000.
fn image(code: &[u8], helper: u64) -> vun_pe::Image {
    use vun_pe::{Export, Image, Import, Machine, Section, SectionFlags};
    Image {
        machine: Machine::X86_64,
        library: false,
        wide: true,
        base: 0,
        entry: 0x1000,
        sections: vec![Section {
            name: b".text".to_vec(),
            address: 0x1000,
            size: code.len() as u64,
            data: code.to_vec(),
            flags: SectionFlags(SectionFlags::CODE.0 | SectionFlags::EXECUTE.0 | SectionFlags::READ.0),
        }],
        exports: vec![Export {
            name: Some(b"helper".to_vec()),
            ordinal: 1,
            address: helper,
            forwarder: None,
        }],
        imports: vec![Import {
            library: b"kernel32.dll".to_vec(),
            name: Some(b"WriteFile".to_vec()),
            ordinal: None,
            slot: 0x2000,
        }],
    }
}

#[test]
fn a_pe_program_is_listed_under_its_entry_and_its_exports() {
    // `call [rip + 0xffa]` through the import slot at 0x2000, `ret`, `ret` at the export.
    let code = [0xff, 0x15, 0xfa, 0x0f, 0x00, 0x00, 0xc3, 0xc3];
    let listed = shown(&listing(&Code::of_pe(&image(&code, 0x1006)).unwrap(), None).unwrap());
    assert_eq!(listed[0], "section .text at 0x1000");
    assert_eq!(listed[1], "0000000000001000 <entry>:");
    assert!(listed[2].ends_with("call qword [rip + 0xffa]  ; 0x2000 <__imp_WriteFile>"), "{}", listed[2]);
    assert_eq!(listed[3], "0000000000001006 <helper>:");
    assert_eq!(listed[4], "    1006:  c3                        ret");
    assert_eq!(listed.len(), 6, "{listed:?}");
}

#[test]
fn an_import_slot_is_named_where_code_refers_to_it() {
    let code = [0x48, 0x8d, 0x05, 0xf9, 0x0f, 0x00, 0x00, 0xc3];
    let lines = listing(&Code::of_pe(&image(&code, 0x1007)).unwrap(), None).unwrap();
    let Line::Inst { target: Some(target), .. } = &lines[2] else { panic!("{lines:?}") };
    assert_eq!(target.to_string(), "0x2000 <__imp_WriteFile>");
}

#[test]
fn a_32_bit_program_is_read_in_protected_mode() {
    let mut image = image(&[0x55, 0x89, 0xe5, 0x5d, 0xc3], 0x1004);
    image.machine = vun_pe::Machine::X86;
    image.wide = false;
    let listed = shown(&listing(&Code::of_pe(&image).unwrap(), Some(b"entry")).unwrap());
    assert!(listed[2].ends_with("push ebp"), "{}", listed[2]);
    assert!(listed[3].ends_with("mov ebp, esp"), "{}", listed[3]);
}

#[test]
fn code_is_read_from_the_format_its_first_bytes_name() {
    assert_eq!(Code::read(b"#!/bin/sh"), Err(Error::Format));
    assert!(matches!(Code::read(b"\x7fELF"), Err(Error::Malformed(_))));
    assert!(matches!(Code::read(b"MZ"), Err(Error::Malformed(_))));
}
