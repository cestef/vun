//! A unit as one relocatable object: functions in `.text`, data by what the program does with
//! it, a symbol per function, datum and import, the relocations of both, and the line table of
//! the functions' code.

use vun_index::IndexVec;
use vun_lir::{Address, DataId, FuncId, ImportKind, Linkage, Unit};

use vun_backend::Lines;
use vun_codegen::Compiled;
use vun_dwarf::{Row, Sequence};
use vun_object::{
    Binding, Contents, Machine, Object, Reloc, RelocKind, Section, SectionId, Symbol, SymbolId, SymbolKind,
};

/// The alignment functions start at.
const FUNCTION_ALIGN: u64 = 16;

/// The section each kind of content goes to, by its conventional name.
const SECTIONS: [(Contents, &[u8]); 5] = [
    (Contents::Code, b".text"),
    (Contents::ReadOnly, b".rodata"),
    (Contents::Relocated, b".data.rel.ro"),
    (Contents::Writable, b".data"),
    (Contents::StackNote, b".note.GNU-stack"),
];

struct Builder {
    object: Object,
    sections: Vec<(Contents, SectionId)>,
}

impl Builder {
    fn section(&mut self, contents: Contents, name: &[u8]) -> SectionId {
        if let Some(&(_, id)) =
            self.sections.iter().find(|(c, id)| *c == contents && self.object.sections[*id].name == name)
        {
            return id;
        }
        let id = self.object.sections.push(Section {
            name: name.to_vec(),
            contents,
            bytes: Vec::new(),
            align: 1,
            relocs: Vec::new(),
        });
        self.sections.push((contents, id));
        id
    }
}

/// What names the producer in debugging information.
const PRODUCER: &str = "vun";

/// The object for `machine` holding `unit`, its functions compiled as `code`, their lines found
/// by `lines`.
#[must_use]
pub(crate) fn assemble<T>(
    unit: &Unit,
    code: &IndexVec<FuncId, Compiled<T>>,
    machine: Machine,
    lines: &dyn Lines,
) -> Object {
    let mut b = Builder { object: Object::new(machine), sections: Vec::new() };
    for (contents, name) in SECTIONS {
        b.section(contents, name);
    }
    let text = b.section(Contents::Code, b".text");
    let named = |linkage: Linkage, own: String| match linkage {
        Linkage::Export(symbol) => (symbol.as_str().as_bytes().to_vec(), Binding::Global),
        Linkage::Internal => (own.into_bytes(), Binding::Local),
    };
    let mut funcs: IndexVec<FuncId, SymbolId> = IndexVec::new();
    for (id, f) in unit.functions.iter() {
        let value = b.object.append(text, &code[id].code.bytes, FUNCTION_ALIGN);
        // An internal function is named by its declaration, as debuggers show it; local symbols
        // may share a name, as instantiations of one declaration do.
        let path = lines.declaration(f.origin);
        let own = if path.is_empty() {
            id.to_string()
        } else {
            path.iter().map(|name| name.as_str()).collect::<Vec<_>>().join(".")
        };
        let (name, binding) = named(f.linkage, own);
        let size = code[id].code.bytes.len() as u64;
        let symbol = Symbol { name, section: Some(text), value, size, kind: SymbolKind::Func, binding };
        funcs.push(b.object.symbols.push(symbol));
    }
    let mut data: IndexVec<DataId, SymbolId> = IndexVec::new();
    let mut placed = Vec::new();
    for (id, d) in unit.data.iter() {
        let contents = match (d.writable, d.relocs.is_empty()) {
            (true, _) => Contents::Writable,
            (false, true) => Contents::ReadOnly,
            (false, false) => Contents::Relocated,
        };
        let name = d.section.map_or_else(
            || SECTIONS.iter().find(|(c, _)| *c == contents).expect("listed").1.to_vec(),
            |s| s.as_str().as_bytes().to_vec(),
        );
        let section = b.section(contents, &name);
        let value = b.object.append(section, &d.bytes, d.align);
        placed.push((section, value));
        let (name, binding) = named(d.linkage, id.to_string());
        let size = d.bytes.len() as u64;
        let symbol = Symbol { name, section: Some(section), value, size, kind: SymbolKind::Object, binding };
        data.push(b.object.symbols.push(symbol));
    }
    let imports: IndexVec<_, SymbolId> = unit
        .imports
        .iter()
        .map(|(_, i)| {
            let kind = match i.kind {
                ImportKind::Func(_) => SymbolKind::Func,
                ImportKind::Data => SymbolKind::Unknown,
            };
            let name = i.symbol.as_str().as_bytes().to_vec();
            b.object.symbols.push(Symbol {
                name,
                section: None,
                value: 0,
                size: 0,
                kind,
                binding: Binding::Global,
            })
        })
        .collect::<Vec<_>>()
        .into();
    let symbol = |a: Address| match a {
        Address::Func(f) => funcs[f],
        Address::Data(d) => data[d],
        Address::Import(i) => imports[i],
    };
    for (id, _) in unit.functions.iter() {
        let start = b.object.symbols[funcs[id]].value;
        for r in &code[id].code.relocs {
            let reloc =
                Reloc { offset: start + r.offset, symbol: symbol(r.target), kind: r.kind, addend: r.addend };
            b.object.sections[text].relocs.push(reloc);
        }
    }
    for ((_, d), (section, start)) in unit.data.iter().zip(placed) {
        for r in &d.relocs {
            let reloc = Reloc {
                offset: start + r.offset,
                symbol: symbol(r.target),
                kind: RelocKind::Abs64,
                addend: r.addend,
            };
            b.object.sections[section].relocs.push(reloc);
        }
    }
    line_table(code, lines, &funcs, &mut b.object, text);
    b.object
}

/// Adds the line table of the functions compiled as `code`, at `funcs`, to `object`: the unit
/// named by the first file it reads.
fn line_table<T>(
    code: &IndexVec<FuncId, Compiled<T>>,
    lines: &dyn Lines,
    funcs: &IndexVec<FuncId, SymbolId>,
    object: &mut Object,
    text: SectionId,
) {
    let mut table = vun_dwarf::Lines::default();
    let mut name = None;
    for (id, compiled) in code.iter() {
        let rows = compiled.marks.iter().filter_map(|&(offset, origin)| {
            let line = lines.line(origin)?;
            name.get_or_insert_with(|| line.file.display().to_string());
            Some(Row { offset, file: table.file(line.file), line: line.line })
        });
        let rows = rows.collect();
        table.sequence(Sequence { symbol: funcs[id], size: compiled.code.bytes.len() as u64, rows });
    }
    table.write(object, text, &name.unwrap_or_default(), PRODUCER);
}
