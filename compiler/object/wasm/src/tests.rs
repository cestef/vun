//! Tests of reading objects: LEB128s as they are written, and what is not an object says why.

use vun_bytes::Out;

use crate::format::{LINKING, LINKING_VERSION, MAGIC, SYMBOL_TABLE, VERSION};
use crate::input::{Coded, Input};
use crate::{Malformed, Object, Section, SymbolFlags, SymbolKind, Symbolic, Wasm};

#[test]
fn leb128s_read_as_they_are_written() {
    let mut out = Out::default();
    out.uleb(624_485);
    out.sleb(-123_456);
    out.uleb_padded(3);
    out.sleb_padded(-2);
    let mut input = Input::new(&out.0);
    assert_eq!(input.uleb(), Ok(624_485));
    assert_eq!(input.sleb(), Ok(-123_456));
    assert_eq!(input.uleb(), Ok(3));
    assert_eq!(input.sleb(), Ok(-2));
    assert!(input.done());
}

#[test]
fn a_leb128_longer_than_five_bytes_is_malformed() {
    let bytes = [0x80, 0x80, 0x80, 0x80, 0x80, 0x00];
    assert_eq!(Input::new(&bytes).uleb(), Err(Malformed::Leb { at: 0 }));
}

/// A module's header, then `sections`.
fn module(sections: &[(Section, Out)]) -> Vec<u8> {
    let mut out = Out::default();
    out.bytes(MAGIC);
    out.u32(VERSION);
    for (section, payload) in sections {
        out.section(*section, payload);
    }
    out.0
}

/// A `linking` section whose symbol table holds one defined function, `f`.
fn linking() -> Out {
    let mut table = Out::default();
    table.uleb(1);
    table.u8(SymbolKind::Function as u8);
    table.uleb(u64::from(SymbolFlags::HIDDEN.0));
    table.uleb(0);
    table.name(b"f");
    let mut payload = Out::default();
    payload.uleb(u64::from(LINKING_VERSION));
    payload.subsection(SYMBOL_TABLE, &table);
    let mut custom = Out::default();
    custom.name(LINKING.as_bytes());
    custom.bytes(&payload.0);
    custom
}

#[test]
fn an_object_reads_as_its_sections_and_symbols_say() {
    let mut types = Out::default();
    types.vector(std::iter::once(crate::FuncType::default()), |out, ty| out.func_type(&ty));
    let mut funcs = Out::default();
    funcs.vector(std::iter::once(0u32), |out, ty| out.uleb(u64::from(ty)));
    let mut code = Out::default();
    code.vector(std::iter::once(()), |out, ()| out.name(&[0, crate::format::op::END]));
    let bytes = module(&[
        (Section::Type, types),
        (Section::Function, funcs),
        (Section::Code, code),
        (Section::Custom, linking()),
    ]);
    let object = Object::read(&bytes).unwrap();
    assert_eq!(object.funcs, [0]);
    assert_eq!(object.code.bodies.len(), 1);
    assert_eq!(object.code.bodies[0], 2..4);
    assert_eq!(object.symbols.len(), 1);
    assert_eq!(object.symbols[0].name.as_deref(), Some("f"));
    assert_eq!(object.symbols[0].what, Symbolic::Function(0));
}

#[test]
fn a_module_without_linking_is_not_relocatable() {
    assert_eq!(Object::read(&module(&[])), Err(Malformed::Linking));
}

#[test]
fn bytes_of_another_format_are_not_a_module() {
    assert_eq!(Object::read(b"\x7fELF\x02\x01\x01\0"), Err(Malformed::Magic));
}

#[test]
fn a_section_an_object_does_not_hold_is_unsupported() {
    let mut memory = Out::default();
    memory.uleb(0);
    let read = Object::read(&module(&[(Section::Memory, memory)]));
    assert_eq!(read, Err(Malformed::Unsupported { what: Coded::Section, code: Section::Memory as u32 }));
}

#[test]
fn a_truncated_section_says_where_it_ends() {
    let mut bytes = module(&[]);
    bytes.extend([Section::Type as u8, 9, 0]);
    assert!(matches!(Object::read(&bytes), Err(Malformed::Truncated { .. })));
}
