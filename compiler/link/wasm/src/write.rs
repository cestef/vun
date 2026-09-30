//! Writing the program: its sections in the order the specification requires, the stack
//! pointer's global starting at the stack's top, the table's entries from its second on, the
//! functions the linker makes after the objects', each section of memory as one segment unless
//! it holds only zeros, which memory starts as, then a name section naming the functions.

use vun_bytes::Out;
use vun_wasm::format::{FUNCREF, FUNCTION_NAMES, MAGIC, MUTABLE, NAME, VERSION, op};
use vun_wasm::{External, Limits, Section, Symbolic, ValType, Wasm};

use crate::layout::{STACK, Value};
use crate::program::Program;
use crate::provided::CALL_CTORS;
use crate::unlinked::Unlinked;

/// What the program exports its memory as, which WASI hosts read.
const MEMORY: &str = "memory";
/// What names the stubs of undefined weak functions, as `wasm-ld` does.
const STUB: &str = "undefined_weak";
/// The table's first entry, which is null: a function's address is never zero.
const FIRST_SLOT: i64 = 1;
/// The program has one table, one memory and one global, and fills its table with one segment.
const ONE: u64 = 1;
/// A data or element segment active in the first memory or table.
const ACTIVE: u64 = 0;
/// A function body without locals.
const NO_LOCALS: u64 = 0;

impl Program<'_> {
    /// The program's bytes.
    ///
    /// # Errors
    /// Where a relocation names a symbol of a kind it cannot.
    pub(crate) fn write(&self) -> Result<Vec<u8>, Unlinked> {
        let (relocated, table) = self.relocated()?;
        let layout = &self.layout;
        let objects = self.symbols.objects;
        let mut out = Out::default();
        out.bytes(MAGIC);
        out.u32(VERSION);
        let mut types = Out::default();
        types.vector(layout.types.iter(), Wasm::func_type);
        out.section(Section::Type, &types);
        let mut imports = Out::default();
        imports.vector(layout.imports.iter(), |out, import| {
            out.name(import.module.as_bytes());
            out.name(import.name.as_bytes());
            out.u8(External::Func as u8);
            out.uleb(u64::from(layout.ty(import.object, import.ty)));
        });
        out.section(Section::Import, &imports);
        let own = layout.funcs.iter().map(|f| layout.ty(f.object, objects[f.object].funcs[f.index as usize]));
        let made = layout.ctors.iter().map(|c| c.ty).chain(layout.stubs.iter().copied());
        let declared: Vec<u32> = own.chain(made).collect();
        let mut funcs = Out::default();
        funcs.vector(declared.iter(), |out, &ty| out.uleb(u64::from(ty)));
        out.section(Section::Function, &funcs);
        let entries = u32::try_from(table.funcs.len()).expect("the table fits u32") + 1;
        let mut tables = Out::default();
        tables.uleb(ONE);
        tables.u8(FUNCREF);
        tables.limits(Limits { min: entries, max: Some(entries) });
        out.section(Section::Table, &tables);
        let mut memory = Out::default();
        memory.uleb(ONE);
        memory.limits(Limits { min: layout.pages(), max: None });
        out.section(Section::Memory, &memory);
        let mut globals = Out::default();
        globals.uleb(ONE);
        globals.u8(ValType::I32.code());
        globals.u8(MUTABLE);
        globals.active(i64::from(STACK));
        out.section(Section::Global, &globals);
        let mut exports = Out::default();
        let exported = std::iter::once((MEMORY, External::Memory, 0))
            .chain(self.exports.iter().map(|&(name, index)| (name, External::Func, index)));
        exports.vector(exported.collect::<Vec<_>>().into_iter(), |out, (name, kind, index)| {
            out.name(name.as_bytes());
            out.u8(kind as u8);
            out.uleb(u64::from(index));
        });
        out.section(Section::Export, &exports);
        if !table.funcs.is_empty() {
            let mut elements = Out::default();
            elements.uleb(ONE);
            elements.uleb(ACTIVE);
            elements.active(FIRST_SLOT);
            elements.vector(table.funcs.iter(), |out, &func| out.uleb(u64::from(func)));
            out.section(Section::Element, &elements);
        }
        out.section(Section::Code, &self.code(&relocated));
        out.section(Section::Data, &self.data(&relocated));
        out.custom(NAME, &self.names());
        Ok(out.0)
    }

    /// The bodies of the objects' functions kept, relocated, then of the linker's: the call of
    /// the initialization functions, and the stubs, which trap.
    fn code(&self, relocated: &[(Out, Out)]) -> Out {
        let layout = &self.layout;
        let objects = self.symbols.objects;
        let mut bodies: Vec<Vec<u8>> = layout
            .funcs
            .iter()
            .map(|f| {
                relocated[f.object].0.0[objects[f.object].code.bodies[f.index as usize].clone()].to_vec()
            })
            .collect();
        if let Some(ctors) = &layout.ctors {
            let mut body = Out::default();
            body.uleb(NO_LOCALS);
            for &call in &ctors.calls {
                body.u8(op::CALL);
                body.uleb(u64::from(call));
            }
            body.u8(op::END);
            bodies.push(body.0);
        }
        let mut stub = Out::default();
        stub.uleb(NO_LOCALS);
        stub.u8(op::UNREACHABLE);
        stub.u8(op::END);
        bodies.extend(layout.stubs.iter().map(|_| stub.0.clone()));
        let mut out = Out::default();
        out.vector(bodies.iter(), |out, body| {
            out.uleb(body.len() as u64);
            out.bytes(body);
        });
        out
    }

    /// Each section of memory holding more than zeros, as one segment of its bytes.
    fn data(&self, relocated: &[(Out, Out)]) -> Out {
        let layout = &self.layout;
        let objects = self.symbols.objects;
        let mut sections: Vec<(u32, Vec<u8>)> = Vec::new();
        let mut last = None;
        for &(seg, at, section) in &layout.segments {
            let bytes = &relocated[seg.object].1.0
                [objects[seg.object].data.segments[seg.index as usize].bytes.clone()];
            match sections.last_mut() {
                Some((start, held)) if last == Some(section) => {
                    held.resize((at - *start) as usize, 0);
                    held.extend_from_slice(bytes);
                }
                _ => sections.push((at, bytes.to_vec())),
            }
            last = Some(section);
        }
        sections.retain(|(_, bytes)| bytes.iter().any(|&b| b != 0));
        let mut out = Out::default();
        out.vector(sections.iter(), |out, (at, bytes)| {
            out.uleb(ACTIVE);
            out.active(i64::from(*at));
            out.name(bytes);
        });
        out
    }

    /// The name section: each function's name, by index.
    fn names(&self) -> Out {
        let layout = &self.layout;
        let imported = layout.imports.iter().map(|import| import.name);
        let mut named: Vec<(u32, &str)> = (0..).zip(imported).collect();
        let symbols =
            self.symbols.objects.iter().zip(&self.symbols.resolved).flat_map(|(object, resolved)| {
                object
                    .symbols
                    .iter()
                    .zip(resolved)
                    .filter(|(s, _)| s.defined() && matches!(s.what, Symbolic::Function(_)))
            });
        for (symbol, &resolved) in symbols {
            if let (Value::Func { index, missing: false }, Some(name)) =
                (layout.value(resolved), symbol.name.as_deref())
            {
                named.push((index, name));
            }
        }
        named.extend(layout.ctors.iter().map(|c| (c.index, CALL_CTORS)));
        named.extend((0..layout.stubs.len()).map(|at| (layout.stub(at), STUB)));
        named.sort_by_key(|&(index, _)| index);
        named.dedup_by_key(|&mut (index, _)| index);
        let mut functions = Out::default();
        functions.vector(named.iter(), |out, &(index, name)| {
            out.uleb(u64::from(index));
            out.name(name.as_bytes());
        });
        let mut out = Out::default();
        out.subsection(FUNCTION_NAMES, &functions);
        out
    }
}
