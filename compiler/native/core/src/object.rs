//! A unit as one relocatable object: functions in `.text`, data by what the program does with
//! it, a symbol per function, datum and import, the relocations of both, and the line table of
//! the functions' code.

use vun_index::IndexVec;
use vun_lir::{Address, DataId, FuncId, ImportId, ImportKind, Linkage, Storage, Unit};

use vun_backend::Lines;
use vun_codegen::Compiled;
use vun_dwarf::{CallFrame, Declared, Fde, Function, Row};
use vun_object::{
    Binding, Contents, Format, Machine, Object, Reloc, SectionId, Symbol, SymbolId, SymbolKind,
};

/// The alignment functions start at.
const FUNCTION_ALIGN: u64 = 16;

/// The section each kind of content goes to, by its conventional name.
const SECTIONS: [(Contents, &[u8]); 6] = [
    (Contents::Code, b".text"),
    (Contents::ReadOnly, b".rodata"),
    (Contents::Relocated, b".data.rel.ro"),
    (Contents::Writable, b".data"),
    (Contents::PerThread, b".tdata"),
    (Contents::StackNote, b".note.GNU-stack"),
];

/// What names the producer in debugging information.
const PRODUCER: &str = "vun";

/// What a unit's object is made of: the unit, its functions compiled as `code`, where their
/// lines are, and how their frames unwind.
pub(crate) struct Assembly<'a, T> {
    pub(crate) unit: &'a Unit,
    pub(crate) code: &'a IndexVec<FuncId, Compiled<T>>,
    pub(crate) lines: &'a dyn Lines,
    pub(crate) frames: &'a CallFrame,
}

impl<T> Assembly<'_, T> {
    /// The object for `machine` in `format`.
    #[must_use]
    pub(crate) fn object(&self, (machine, format): (Machine, Format)) -> Object {
        let mut object = Object::new(machine, format);
        for (contents, name) in SECTIONS {
            object.section_named(name, contents);
        }
        let text = object.section_named(b".text", Contents::Code);
        if let Some(code) = machine.code() {
            object.map(text, 0, code);
        }
        let funcs = self.functions(&mut object, text);
        let (data, placed) = self.data(&mut object);
        let imports = self.imports(&mut object);
        let symbol = |a: Address| match a {
            Address::Func(f) => funcs[f],
            Address::Data(d) => data[d],
            Address::Import(i) => imports[i],
        };
        for (id, _) in self.unit.functions.iter() {
            let start = object.symbols[funcs[id]].value;
            for r in &self.code[id].code.relocs {
                let reloc = Reloc {
                    offset: start + r.offset,
                    symbol: symbol(r.target),
                    kind: r.kind,
                    addend: r.addend,
                };
                object.sections[text].relocs.push(reloc);
            }
        }
        for ((_, d), (section, start)) in self.unit.data.iter().zip(placed) {
            for r in &d.relocs {
                let (symbol, addend) = (symbol(r.target), r.addend);
                let reloc =
                    Reloc { offset: start + r.offset, symbol, kind: object.machine.address(), addend };
                object.sections[section].relocs.push(reloc);
                if object.machine.implicit_addends() {
                    let at = usize::try_from(reloc.offset).expect("data fits memory");
                    let width = usize::from(object.machine.address_bytes());
                    object.sections[section].bytes[at..at + width]
                        .copy_from_slice(&addend.to_le_bytes()[..width]);
                }
            }
        }
        self.debug_info(&funcs, &mut object, text);
        // Mach-O describes frames otherwise (compact unwind information), which is left for later.
        if format == Format::Elf {
            let fdes = self.code.iter().filter(|(_, c)| !c.code.bytes.is_empty()).map(|(id, c)| Fde {
                symbol: funcs[id],
                size: c.code.bytes.len() as u64,
                steps: c.unwind.clone(),
            });
            self.frames.write(&mut object, &fdes.collect::<Vec<_>>());
            // Mach-O keeps naming symbols: its linker splits sections at them.
            object.bind_locally();
        } else {
            object.thread_variables();
        }
        object
    }

    /// The name and binding of a symbol of `linkage`, named `own` when the unit keeps it.
    fn named(linkage: Linkage, own: String) -> (Vec<u8>, Binding) {
        match linkage {
            Linkage::Export(symbol) => (symbol.as_str().as_bytes().to_vec(), Binding::Global),
            Linkage::Internal => (own.into_bytes(), Binding::Local),
        }
    }

    /// Appends the functions' code to `text`, a symbol for each.
    fn functions(&self, object: &mut Object, text: SectionId) -> IndexVec<FuncId, SymbolId> {
        let mut funcs = IndexVec::new();
        for (id, f) in self.unit.functions.iter() {
            let bytes = &self.code[id].code.bytes;
            let value = object.append(text, bytes, FUNCTION_ALIGN);
            // An internal function is named by its declaration, as debuggers show it; local symbols
            // may share a name, as instantiations of one declaration do.
            let own = self.lines.name(f.origin).unwrap_or_else(|| id.to_string());
            let (name, binding) = Self::named(f.linkage, own);
            let size = bytes.len() as u64;
            let symbol = Symbol { name, section: Some(text), value, size, kind: SymbolKind::Func, binding };
            funcs.push(object.symbols.push(symbol));
        }
        funcs
    }

    /// Appends each datum to the section of what the program does with it, a symbol for each;
    /// with where each was placed.
    fn data(&self, object: &mut Object) -> (IndexVec<DataId, SymbolId>, Vec<(SectionId, u64)>) {
        let mut data = IndexVec::new();
        let mut placed = Vec::new();
        for (id, d) in self.unit.data.iter() {
            let contents = match (d.storage, d.writable, d.relocs.is_empty()) {
                (Storage::Thread, ..) => Contents::PerThread,
                (Storage::Program, true, _) => Contents::Writable,
                (Storage::Program, false, true) => Contents::ReadOnly,
                (Storage::Program, false, false) => Contents::Relocated,
            };
            let kind = match d.storage {
                Storage::Program => SymbolKind::Object,
                Storage::Thread => SymbolKind::Tls,
            };
            let name = d.section.map_or_else(
                || SECTIONS.iter().find(|(c, _)| *c == contents).expect("listed").1.to_vec(),
                |s| s.as_str().as_bytes().to_vec(),
            );
            let section = object.section_named(&name, contents);
            let value = object.append(section, &d.bytes, d.align);
            placed.push((section, value));
            let (name, binding) = Self::named(d.linkage, id.to_string());
            let size = d.bytes.len() as u64;
            let symbol = Symbol { name, section: Some(section), value, size, kind, binding };
            data.push(object.symbols.push(symbol));
        }
        (data, placed)
    }

    /// A symbol for each import, defined elsewhere.
    fn imports(&self, object: &mut Object) -> IndexVec<ImportId, SymbolId> {
        let imports = self.unit.imports.iter().map(|(_, i)| {
            let kind = match i.kind {
                ImportKind::Func(_) => SymbolKind::Func,
                ImportKind::Data => SymbolKind::Unknown,
            };
            let name = i.symbol.as_str().as_bytes().to_vec();
            object.symbols.push(Symbol {
                name,
                section: None,
                value: 0,
                size: 0,
                kind,
                binding: Binding::Global,
            })
        });
        imports.collect::<Vec<_>>().into()
    }

    /// Adds the debugging information of the functions, at `funcs`, to `object`: an entry per
    /// function and their line table, in the unit named by the first file it reads.
    fn debug_info(&self, funcs: &IndexVec<FuncId, SymbolId>, object: &mut Object, text: SectionId) {
        let (code, lines) = (self.code, self.lines);
        let mut table = vun_dwarf::Lines::default();
        let mut name = None;
        for (id, compiled) in code.iter() {
            let rows = compiled.marks.iter().filter_map(|&(offset, origin)| {
                let line = lines.line(origin)?;
                name.get_or_insert_with(|| line.file.display().to_string());
                Some(Row { offset, file: table.file(line.file), line: line.line })
            });
            let rows = rows.collect();
            let declared = lines.line(self.unit.functions[id].origin);
            let declared = declared.map(|l| Declared { file: table.file(l.file), line: l.line });
            let symbol = funcs[id];
            table.function(Function {
                symbol,
                size: compiled.code.bytes.len() as u64,
                // As its symbol names it, which is what debuggers and profilers show.
                name: String::from_utf8_lossy(&object.symbols[symbol].name).into_owned(),
                declared,
                rows,
            });
        }
        table.write(object, text, &name.unwrap_or_default(), PRODUCER);
    }
}
