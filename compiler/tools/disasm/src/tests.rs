use vun_elf::{Binding, Class, File, Kind, Machine, Section, SectionFlags, SectionKind, Symbol, SymbolKind};

use super::*;

/// An object whose code is `code`, with symbols `(name, offset, size)` placed in it.
fn object(machine: Machine, code: &[u8], symbols: &[(&[u8], u64, u64)]) -> File {
    let symbols: Vec<Symbol> = symbols
        .iter()
        .map(|&(name, value, size)| Symbol {
            name: name.to_vec(),
            kind: SymbolKind::Func,
            binding: Binding::Global,
            section: 1,
            value,
            size,
        })
        .collect();
    let (table, strings) = Symbol::table(Class::Elf64, &symbols);
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

fn shown(lines: &[Line]) -> Vec<String> {
    lines.iter().map(ToString::to_string).collect()
}

/// `push rbp; mov rbp, rsp; pop rbp; ret`.
const X86: [u8; 6] = [0x55, 0x48, 0x89, 0xe5, 0x5d, 0xc3];

#[test]
fn x86_64_code_is_listed_instruction_by_instruction_under_its_labels() {
    let file = object(Machine::X86_64, &X86, &[(b"f", 0, 6)]);
    let listed = shown(&listing(&file, None).unwrap());
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
    let listed = shown(&listing(&file, None).unwrap());
    assert!(listed[1].ends_with("ret"), "{}", listed[1]);
    assert!(listed[2].ends_with(".byte 0xff"), "{}", listed[2]);
}

#[test]
fn bytes_that_start_no_instruction_are_listed_as_data_one_at_a_time() {
    let lines = listing(&object(Machine::X86_64, &[0x06, 0xc3], &[]), None).unwrap();
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
    let lines = listing(&file, Some(b"g")).unwrap();
    let texts: Vec<_> = lines
        .iter()
        .filter_map(|l| if let Line::Inst { text, .. } = l { Some(text.as_str()) } else { None })
        .collect();
    assert_eq!(texts, ["mov rbp, rsp", "pop rbp", "ret"]);
    assert_eq!(listing(&file, Some(b"f")).unwrap().len(), 3);
}

#[test]
fn a_name_that_labels_no_code_is_refused() {
    let file = object(Machine::X86_64, &X86, &[(b"f", 0, 6)]);
    assert_eq!(listing(&file, Some(b"h")), Err(Error::Missing(b"h".to_vec())));
}

#[test]
fn a_machine_whose_instructions_are_not_read_is_refused() {
    let file = object(Machine::Arm, &[0; 4], &[]);
    assert_eq!(listing(&file, None), Err(Error::Machine(Machine::Arm)));
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
    let lines = listing(&program(), None).unwrap();
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
    let lines = listing(&file, Some(b"main")).unwrap();
    let Some(Line::Inst { target: Some(target), .. }) = lines.last() else { panic!("{lines:?}") };
    assert_eq!(target.to_string(), "0x1002 <main+0x2>");
}

#[test]
fn an_object_names_no_target() {
    let code = [0xe8, 0x00, 0x00, 0x00, 0x00];
    let lines = listing(&object(Machine::X86_64, &code, &[(b"f", 0, 5)]), None).unwrap();
    assert!(matches!(&lines[2], Line::Inst { target: Some(Target { address: 5, symbol: None }), .. }));
}

#[test]
fn an_instruction_ends_where_the_next_label_begins() {
    // A stray zero byte before `push rbp; ret` at the label `f`: `00 55 48` would be one
    // instruction, swallowing the start of `f`.
    let file = object(Machine::X86_64, &[0x00, 0x55, 0xc3], &[(b"f", 1, 2)]);
    let lines = listing(&file, None).unwrap();
    assert_eq!(lines[1], Line::Data { address: 0, byte: 0 });
    assert_eq!(lines[2], Line::Label { address: 1, name: b"f".to_vec() });
    assert!(matches!(&lines[3], Line::Inst { text, .. } if text == "push rbp"));
}
