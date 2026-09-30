//! The wasm linker on objects the wasm backend writes: what links is valid, as an independent
//! validator reads it, and holds what its symbols lead to; what does not link says why.

use vun_backend_wasm::module::op::Op;
use vun_backend_wasm::module::{
    Binding, Def, Func, FuncType, Kind, Linker, Module, Segment, Sym, SymbolId, ValType,
};
use vun_intern::Symbol;
use wasmparser::{Parser, Payload};

use vun_link_wasm::{STACK, Unlinked, WasmLinker};

const WASI: &str = "wasi_snapshot_preview1";
const ENTRY: &str = "_start";

/// A function symbol of `m` of type `ty`, defined here with `body` or imported from `from`.
fn func(
    m: &mut Module,
    name: &str,
    ty: FuncType,
    binding: Binding,
    body: Option<Vec<Op>>,
    from: &str,
) -> SymbolId {
    let ty = m.ty(ty);
    let def = match body {
        Some(body) => {
            m.funcs.push(Func { ty, locals: Vec::new(), body, marks: Vec::new() });
            Def::Here(m.funcs.len() - 1)
        }
        None => Def::Imported { module: Symbol::intern(from) },
    };
    m.symbols.push(Sym { name: Symbol::intern(name), kind: Kind::Func { ty, def }, binding })
}

fn data(m: &mut Module, name: &str, segment: Option<Segment>, binding: Binding) -> SymbolId {
    let at = segment.map(|segment| {
        m.segments.push(segment);
        m.segments.len() - 1
    });
    m.symbols.push(Sym { name: Symbol::intern(name), kind: Kind::Data(at), binding })
}

fn segment(bytes: &[u8], section: Option<&str>) -> Segment {
    Segment {
        bytes: bytes.to_vec(),
        align: 4,
        writable: true,
        section: section.map(Symbol::intern),
        relocs: Vec::new(),
    }
}

fn gives(result: ValType) -> FuncType {
    FuncType { params: Vec::new(), results: vec![result] }
}

fn takes(param: ValType) -> FuncType {
    FuncType { params: vec![param], results: Vec::new() }
}

/// The program's entry: calls `helper`, takes the address of `message` and of `helper`, then
/// exits through WASI.
fn main_object() -> Vec<u8> {
    let mut m = Module::default();
    m.linked(Linker::StackPointer);
    m.linked(Linker::Table);
    let helper = func(&mut m, "helper", gives(ValType::I32), Binding::Hidden, None, vun_wasm::ENV);
    let exit = func(&mut m, "proc_exit", takes(ValType::I32), Binding::Hidden, None, WASI);
    let message = data(&mut m, "message", None, Binding::Hidden);
    let body = vec![
        Op::Call(helper),
        Op::Drop,
        Op::Address(message, 2),
        Op::Drop,
        Op::Address(helper, 0),
        Op::Drop,
        Op::I32Const(0),
        Op::Call(exit),
    ];
    func(&mut m, ENTRY, FuncType::default(), Binding::Exported, Some(body), "");
    assert_eq!(m.verify(), Ok(()));
    m.write()
}

/// Defines `helper`, `message`, a table of their addresses in the section `registry`, and a
/// function nothing calls.
fn library_object() -> Vec<u8> {
    let mut m = Module::default();
    m.linked(Linker::StackPointer);
    let helper =
        func(&mut m, "helper", gives(ValType::I32), Binding::Hidden, Some(vec![Op::I32Const(7)]), "");
    let message = data(&mut m, "message", Some(segment(b"hello\0\0\0", None)), Binding::Hidden);
    let mut table = segment(&[0; 8], Some("registry"));
    table.relocs = vec![(0, helper, 0), (4, message, 1)];
    data(&mut m, "table", Some(table), Binding::Exported);
    func(&mut m, "unused", gives(ValType::I32), Binding::Hidden, Some(vec![Op::I32Const(1)]), "");
    assert_eq!(m.verify(), Ok(()));
    m.write()
}

/// What a test reads of a program.
#[derive(Debug, Default)]
pub(super) struct Read {
    pub(super) imports: Vec<(String, String)>,
    pub(super) exports: Vec<String>,
    pub(super) functions: usize,
    pub(super) elements: Vec<u32>,
    pub(super) data: Vec<(u32, Vec<u8>)>,
}

/// Validates `program` as wasmparser does, and reads it.
pub(super) fn read(program: &[u8]) -> Read {
    wasmparser::Validator::new().validate_all(program).unwrap();
    let mut read = Read::default();
    for payload in Parser::new(0).parse_all(program) {
        match payload.unwrap() {
            Payload::ImportSection(imports) => {
                for import in imports.into_imports() {
                    let import = import.unwrap();
                    read.imports.push((import.module.to_owned(), import.name.to_owned()));
                }
            }
            Payload::ExportSection(exports) => {
                read.exports = exports.into_iter().map(|e| e.unwrap().name.to_owned()).collect();
            }
            Payload::FunctionSection(funcs) => read.functions = funcs.count() as usize,
            Payload::ElementSection(elements) => {
                for element in elements {
                    if let wasmparser::ElementItems::Functions(funcs) = element.unwrap().items {
                        read.elements.extend(funcs.into_iter().map(Result::unwrap));
                    }
                }
            }
            Payload::DataSection(segments) => {
                for segment in segments {
                    let segment = segment.unwrap();
                    let wasmparser::DataKind::Active { offset_expr, .. } = segment.kind else { continue };
                    let Ok(wasmparser::Operator::I32Const { value }) =
                        offset_expr.get_operators_reader().read()
                    else {
                        continue;
                    };
                    read.data.push((value.cast_unsigned(), segment.data.to_vec()));
                }
            }
            _ => {}
        }
    }
    read
}

/// The four bytes of `data` at `address`, as a little-endian word.
fn word(data: &[(u32, Vec<u8>)], address: u32) -> u32 {
    let (start, bytes) = data
        .iter()
        .find(|(start, bytes)| (*start..*start + u32::try_from(bytes.len()).unwrap()).contains(&address))
        .unwrap();
    let at = (address - start) as usize;
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

#[test]
fn objects_link_into_a_valid_program_of_what_its_entry_reaches() {
    let program = WasmLinker::program(&[main_object(), library_object()], Some(ENTRY)).unwrap();
    let read = read(&program);
    assert_eq!(read.imports, [(WASI.to_owned(), "proc_exit".to_owned())]);
    assert_eq!(read.exports, ["memory", ENTRY]);
    // `_start` and `helper`; `unused` is left out.
    assert_eq!(read.functions, 2);
    // `helper`, after the import and `_start`, is the table's one function.
    assert_eq!(read.elements, [2]);
}

#[test]
fn a_segment_nothing_reaches_is_left_out_and_one_reached_is_relocated() {
    let program = WasmLinker::program(&[main_object(), library_object()], Some(ENTRY)).unwrap();
    let read = read(&program);
    // Only `message` is reached: the table in `registry` is not.
    assert_eq!(read.data.len(), 1);
    assert_eq!(&read.data[0].1[..5], b"hello");
    assert!(read.data[0].0 >= STACK);
}

#[test]
fn a_kept_segment_holds_the_addresses_it_names() {
    let mut m = Module::default();
    m.linked(Linker::StackPointer);
    let table = data(&mut m, "table", None, Binding::Hidden);
    let body = vec![Op::Address(table, 0), Op::Drop];
    func(&mut m, ENTRY, FuncType::default(), Binding::Exported, Some(body), "");
    let program = WasmLinker::program(&[m.write(), library_object()], Some(ENTRY)).unwrap();
    let read = read(&program);
    let (message, _) = *read.data.iter().find(|(_, bytes)| bytes.starts_with(b"hello")).unwrap();
    let (table, _) = *read.data.iter().find(|(at, _)| *at != message).unwrap();
    // The table in `registry` holds `helper`'s slot, then `message`'s address plus one.
    assert_eq!(word(&read.data, table), 1);
    assert_eq!(word(&read.data, table + 4), message + 1);
    assert_eq!(read.elements.len(), 1);
}

#[test]
fn a_symbol_no_object_defines_is_undefined() {
    let linked = WasmLinker::program(&[main_object()], Some(ENTRY));
    assert_eq!(linked, Err(Unlinked::Undefined("helper".to_owned())));
}

#[test]
fn a_symbol_two_objects_define_is_a_duplicate() {
    let linked = WasmLinker::program(&[main_object(), library_object(), library_object()], Some(ENTRY));
    assert_eq!(linked, Err(Unlinked::Duplicate("helper".to_owned())));
}

#[test]
fn an_entry_no_object_defines_is_undefined() {
    let linked = WasmLinker::program(&[library_object()], Some(ENTRY));
    assert_eq!(linked, Err(Unlinked::Undefined(ENTRY.to_owned())));
}

#[test]
fn bytes_that_are_no_object_are_malformed() {
    let linked = WasmLinker::program(&[b"\0elf".to_vec()], None);
    assert!(matches!(linked, Err(Unlinked::Malformed { input: 0, why: vun_wasm::Malformed::Magic })));
}

#[test]
fn a_library_exports_what_its_objects_export() {
    let program = WasmLinker::program(&[library_object()], None).unwrap();
    let read = read(&program);
    assert_eq!(read.exports, ["memory"]);
    let mut m = Module::default();
    func(&mut m, "calc_add", gives(ValType::I32), Binding::Exported, Some(vec![Op::I32Const(42)]), "");
    let read = self::read(&WasmLinker::program(&[m.write()], None).unwrap());
    assert_eq!(read.exports, ["memory", "calc_add"]);
}
