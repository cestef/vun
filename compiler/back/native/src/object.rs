//! A unit as one relocatable object: functions in `.text`, data by what the program does with
//! it, a symbol per function, datum and import, and the relocations of both.

use vun_index::IndexVec;
use vun_lir::{Address, DataId, FuncId, ImportKind, Linkage, Unit};

use crate::Code;
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

/// The object holding `unit`, its functions compiled as `code`.
#[must_use]
pub(crate) fn assemble(unit: &Unit, code: &IndexVec<FuncId, Code>) -> Object {
    let mut b = Builder { object: Object::new(Machine::X86_64), sections: Vec::new() };
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
        let (name, binding) = named(f.linkage, id.to_string());
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
    b.object
}
