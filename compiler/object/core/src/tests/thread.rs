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
