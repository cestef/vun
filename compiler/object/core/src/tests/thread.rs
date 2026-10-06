//! Storage per thread: reached through a symbol of its own on ELF, and through a descriptor on
//! Mach-O.

use crate::{Binding, Contents, Format, Machine, Object, Reloc, RelocKind, Symbol, SymbolId, SymbolKind};

/// A function reaching storage per thread exported as `count`, the storage's four bytes.
fn object(format: Format) -> (Object, SymbolId) {
    let mut object = Object::new(Machine::Aarch64, format);
    let text = object.section(b".text", Contents::Code);
    let data = object.section(b".tdata", Contents::PerThread);
    object.append(text, &[0; 8], 4);
    let at = object.append(data, &[1, 0, 0, 0], 4);
    let count = object.symbols.push(Symbol {
        name: b"count".to_vec(),
        section: Some(data),
        value: at,
        size: 4,
        kind: SymbolKind::Tls,
        binding: Binding::Global,
    });
    let reloc = |offset, kind| Reloc { offset, symbol: count, kind, addend: 0 };
    object.sections[text].relocs =
        vec![reloc(0, RelocKind::ThreadVariablePage21), reloc(4, RelocKind::ThreadVariablePageOffset64)];
    (object, count)
}

#[test]
fn a_relocation_bound_locally_names_a_symbol_of_the_storage_never_its_section() {
    let (mut object, count) = object(Format::Elf);
    object.bind_locally();
    for r in &object.sections.iter().next().expect("text").1.relocs {
        let symbol = &object.symbols[r.symbol];
        assert_ne!(r.symbol, count, "the exported symbol could be another module's");
        assert_eq!(
            (symbol.kind, symbol.binding, &symbol.name[..]),
            (SymbolKind::Tls, Binding::Local, &b"count"[..])
        );
    }
}

#[test]
fn a_thread_local_variable_names_its_descriptor_which_names_its_initial_bytes() {
    let (mut object, count) = object(Format::MachO);
    object.thread_variables();
    let variable = &object.symbols[count];
    let variables = variable.section.expect("defined");
    let section = &object.sections[variables];
    assert_eq!((section.contents, variable.size, section.bytes.len()), (Contents::ThreadVariables, 24, 24));
    let named: Vec<(u64, &[u8])> =
        section.relocs.iter().map(|r| (r.offset, &object.symbols[r.symbol].name[..])).collect();
    assert_eq!(named, [(0, &b"_tlv_bootstrap"[..]), (16, &b"count$tlv$init"[..])]);
}

#[test]
fn a_relocation_on_arm_keeps_naming_its_symbol_for_linkers_reading_addends_from_the_field() {
    let mut object = Object::new(Machine::Arm(crate::Float::Hard), Format::Elf);
    let text = object.section(b".text", Contents::Code);
    object.append(text, &[0; 8], 4);
    let callee = object.symbols.push(Symbol {
        name: b"callee".to_vec(),
        section: Some(text),
        value: 4,
        size: 4,
        kind: SymbolKind::Func,
        binding: Binding::Local,
    });
    object.sections[text].relocs =
        vec![Reloc { offset: 0, symbol: callee, kind: RelocKind::ThumbCall, addend: -4 }];
    object.bind_locally();
    let relocs = &object.sections[text].relocs;
    assert_eq!((relocs[0].symbol, relocs[0].addend), (callee, -4));
}

#[test]
fn a_relocation_through_the_global_offset_table_keeps_naming_its_symbol() {
    let mut object = Object::new(Machine::Aarch64, Format::Elf);
    let data = object.section(b".data", Contents::Writable);
    let text = object.section(b".text", Contents::Code);
    object.append(text, &[0; 8], 4);
    let at = object.append(data, &[0; 8], 8);
    let held = object.symbols.push(Symbol {
        name: b"held".to_vec(),
        section: Some(data),
        value: at + 8,
        size: 8,
        kind: SymbolKind::Object,
        binding: Binding::Local,
    });
    let reloc = |offset, kind| Reloc { offset, symbol: held, kind, addend: 0 };
    object.sections[text].relocs = vec![reloc(0, RelocKind::GotPage21), reloc(4, RelocKind::Page21)];
    object.bind_locally();
    let relocs = &object.sections[text].relocs;
    // The entry in the table is the symbol's; the page of a direct reference is the section's.
    assert_eq!((relocs[0].symbol, relocs[0].addend), (held, 0));
    assert_ne!(relocs[1].symbol, held);
}
