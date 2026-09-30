//! Writing a module: its sections in the order the specification requires, then the custom
//! sections a linker reads.

use vun_bytes::Out;
use vun_index::Idx;
use vun_wasm::format::{FUNCREF, LINKING, MAGIC, MUTABLE, RELOC, VERSION, op};
use vun_wasm::{External, Limits, PAGE, Section, Wasm};

use super::code::Body;
use super::link::{Reloc, RelocKind, relocs};
use super::{Def, ENV, Func, Kind, Linker, Module, SymbolId, TypeId, ValType};

/// What the linker defines for every object's memory.
const MEMORY: &[u8] = b"__linear_memory";

impl TypeId {
    pub(super) fn number(self) -> u32 {
        u32::try_from(self.index()).expect("types fit u32")
    }
}

/// The sections written so far, counted as relocation sections name them.
struct Sections {
    out: Out,
    count: u32,
}

impl Sections {
    /// Appends a section; gives its number.
    fn add(&mut self, section: Section, payload: &Out) -> u32 {
        self.out.section(section, payload);
        self.count += 1;
        self.count - 1
    }

    fn custom(&mut self, name: &str, payload: &Out) {
        self.out.custom(name, payload);
        self.count += 1;
    }
}

impl Module {
    /// The object file.
    ///
    /// # Panics
    /// Panics on a module [`Module::verify`] rejects.
    #[must_use]
    pub fn write(&self) -> Vec<u8> {
        let functions = self.functions();
        let placed = self.placed();
        let mut sections = Sections { out: Out::default(), count: 0 };
        sections.out.bytes(MAGIC);
        sections.out.u32(VERSION);
        let mut types = Out::default();
        types.vector(self.types.iter(), |out, (_, ty)| out.func_type(ty));
        sections.add(Section::Type, &types);
        sections.add(Section::Import, &self.imports(placed.last().copied().unwrap_or(0)));
        let mut declared = Out::default();
        declared.uleb(self.funcs.len() as u64);
        for f in &self.funcs {
            declared.uleb(u64::from(f.ty.number()));
        }
        sections.add(Section::Function, &declared);
        let (code, code_relocs) = self.code(&functions, &placed);
        let code_section = sections.add(Section::Code, &code);
        let (data, data_relocs) = self.data(&placed);
        let data_section = (!self.segments.is_empty()).then(|| sections.add(Section::Data, &data));
        sections.custom(LINKING, &self.linking(&functions));
        if !code_relocs.is_empty() {
            sections.custom(&[RELOC, "CODE"].concat(), &relocs(code_section, &code_relocs));
        }
        if let Some(section) = data_section.filter(|_| !data_relocs.is_empty()) {
            sections.custom(&[RELOC, "DATA"].concat(), &relocs(section, &data_relocs));
        }
        sections.out.0
    }

    /// Each segment's address in the object's memory, then the end of the last.
    fn placed(&self) -> Vec<u64> {
        let mut at: u64 = 0;
        let mut placed = Vec::with_capacity(self.segments.len() + 1);
        for s in &self.segments {
            at = at.next_multiple_of(s.align.max(1));
            placed.push(at);
            at += s.bytes.len() as u64;
        }
        placed.push(at);
        placed
    }

    fn imports(&self, memory: u64) -> Out {
        let mut entries: Vec<Out> = Vec::new();
        let mut import = |module: &[u8], name: &[u8], rest: &dyn Fn(&mut Out)| {
            let mut out = Out::default();
            out.name(module);
            out.name(name);
            rest(&mut out);
            entries.push(out);
        };
        import(ENV.as_bytes(), MEMORY, &|out| {
            out.u8(External::Memory as u8);
            out.limits(Limits {
                min: u32::try_from(memory.div_ceil(PAGE)).expect("memory fits 4 GiB"),
                max: None,
            });
        });
        if self.linker(Linker::StackPointer).is_some() {
            import(ENV.as_bytes(), Linker::StackPointer.name().as_bytes(), &|out| {
                out.u8(External::Global as u8);
                out.u8(ValType::I32.code());
                out.u8(MUTABLE);
            });
        }
        if self.linker(Linker::Table).is_some() {
            import(ENV.as_bytes(), Linker::Table.name().as_bytes(), &|out| {
                out.u8(External::Table as u8);
                out.u8(FUNCREF);
                out.limits(Limits { min: 0, max: None });
            });
        }
        for (_, sym) in self.symbols.iter() {
            if let Kind::Func { ty, def: Def::Imported { module } } = &sym.kind {
                import(module.as_str().as_bytes(), sym.name.as_str().as_bytes(), &|out| {
                    out.u8(External::Func as u8);
                    out.uleb(u64::from(ty.number()));
                });
            }
        }
        let mut out = Out::default();
        out.uleb(entries.len() as u64);
        for e in entries {
            out.bytes(&e.0);
        }
        out
    }

    /// The code section and its relocations, at offsets of its payload.
    fn code(&self, functions: &[SymbolId], placed: &[u64]) -> (Out, Vec<Reloc>) {
        let mut out = Out::default();
        let mut all = Vec::new();
        out.uleb(self.funcs.len() as u64);
        for f in &self.funcs {
            let (body, relocs) = self.body(f, functions, placed);
            out.uleb(body.here());
            let start = u32::try_from(out.here()).expect("code fits 4 GiB");
            all.extend(relocs.into_iter().map(|r| Reloc { offset: start + r.offset, ..r }));
            out.bytes(&body.0);
        }
        (out, all)
    }

    fn body(&self, f: &Func, functions: &[SymbolId], placed: &[u64]) -> (Out, Vec<Reloc>) {
        let mut body = Body { out: Out::default(), relocs: Vec::new(), module: self, functions, placed };
        let mut runs: Vec<(u32, ValType)> = Vec::new();
        for &ty in &f.locals {
            match runs.last_mut() {
                Some((n, last)) if *last == ty => *n += 1,
                _ => runs.push((1, ty)),
            }
        }
        body.out.uleb(runs.len() as u64);
        for (n, ty) in runs {
            body.out.uleb(u64::from(n));
            body.out.u8(ty.code());
        }
        for op in &f.body {
            body.op(op);
        }
        body.out.u8(op::END);
        (body.out, body.relocs)
    }

    fn data(&self, placed: &[u64]) -> (Out, Vec<Reloc>) {
        let mut out = Out::default();
        let mut all = Vec::new();
        out.uleb(self.segments.len() as u64);
        for (s, &at) in self.segments.iter().zip(placed) {
            out.u8(0);
            out.active(i64::try_from(at).expect("memory fits"));
            out.uleb(s.bytes.len() as u64);
            let start = u32::try_from(out.here()).expect("data fits 4 GiB");
            for &(offset, sym, addend) in &s.relocs {
                let kind = match self.symbols[sym].kind {
                    Kind::Func { .. } => RelocKind::TableIndexI32,
                    _ => RelocKind::MemoryAddrI32,
                };
                all.push(Reloc { kind, offset: start + offset, index: sym.number(), addend });
            }
            out.bytes(&s.bytes);
        }
        (out, all)
    }
}
