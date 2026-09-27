//! A unit as a module: a symbol per function reached, import and datum, the types of the
//! signatures they have and the indirect calls make, then every function's body, in parallel.

mod abi;
mod call;
mod control;
mod func;
mod inst;
mod select;

use vun_backend::{Lack, Lines, Missing};
use vun_index::{Idx, IndexVec};
use vun_intern::Symbol;
use vun_lir::analysis::Reached;
use vun_lir::{Address, Callee, DataId, FuncId, ImportId, ImportKind, InstKind, Linkage, Unit};
use vun_origin::OriginId;
use vun_par::Parallel;

use crate::module::{Binding, Def, ENV, Func, Kind, Linker, Module, Segment, Sym, SymbolId};
use abi::Passing;
use control::{Emit, Shape};
use func::Lower;

/// What every function's lowering reads.
struct Cx<'a> {
    unit: &'a Unit,
    module: Module,
    /// The symbol of each function reached; one inlined everywhere has none.
    funcs: IndexVec<FuncId, Option<SymbolId>>,
    imports: IndexVec<ImportId, SymbolId>,
    data: IndexVec<DataId, SymbolId>,
    /// How each function reached is passed.
    passings: IndexVec<FuncId, Option<Passing>>,
    /// How each imported function is passed.
    imported: IndexVec<ImportId, Option<Passing>>,
    stack_pointer: SymbolId,
}

impl Cx<'_> {
    fn passing(&self, f: FuncId) -> &Passing {
        self.passings[f].as_ref().expect("a called function is reached")
    }

    fn symbol(&self, address: Address) -> SymbolId {
        match address {
            Address::Func(f) => self.funcs[f].expect("a function whose address is taken is reached"),
            Address::Data(d) => self.data[d],
            Address::Import(i) => self.imports[i],
        }
    }
}

/// The module of `unit`.
///
/// # Errors
/// Returns the first thing it holds that wasm cannot express.
pub(super) fn module(unit: &Unit, lines: &dyn Lines) -> Result<Module, Lack> {
    let mut module = Module::default();
    let stack_pointer = module.linked(Linker::StackPointer);
    module.linked(Linker::Table);
    let reached = Reached::of(unit);
    let named = |linkage: Linkage, origin: OriginId, id: &dyn ToString| match linkage {
        Linkage::Export(symbol) => (symbol, Binding::Exported),
        Linkage::Internal => {
            (Symbol::intern(&lines.name(origin).unwrap_or_else(|| id.to_string())), Binding::Local)
        }
    };
    let mut funcs = IndexVec::new();
    let mut passings = IndexVec::new();
    let mut defined = 0;
    for (id, f) in unit.functions.iter() {
        if !reached.contains(id) {
            funcs.push(None);
            passings.push(None);
            continue;
        }
        let passing = Passing::of(&f.signature).map_err(|m| m.at(f.origin))?;
        let ty = module.ty(passing.ty.clone());
        let (name, binding) = named(f.linkage, f.origin, &id.index());
        let kind = Kind::Func { ty, def: Def::Here(defined) };
        funcs.push(Some(module.symbols.push(Sym { name, kind, binding })));
        passings.push(Some(passing));
        defined += 1;
    }
    let mut imports = IndexVec::new();
    let mut imported = IndexVec::new();
    for (_, import) in unit.imports.iter() {
        let (kind, passing) = match &import.kind {
            ImportKind::Func(signature) => {
                let passing = Passing::of(signature).map_err(|m| m.at(import.origin))?;
                let ty = module.ty(passing.ty.clone());
                let from = import.library.unwrap_or_else(|| Symbol::intern(ENV));
                (Kind::Func { ty, def: Def::Imported { module: from } }, Some(passing))
            }
            ImportKind::Data => (Kind::Data(None), None),
        };
        imports.push(module.symbols.push(Sym { name: import.symbol, kind, binding: Binding::Hidden }));
        imported.push(passing);
    }
    let mut data = IndexVec::new();
    for (id, d) in unit.data.iter() {
        if d.section.is_some() {
            return Err(Missing::Section.at(d.origin));
        }
        let segment =
            Segment { bytes: d.bytes.clone(), align: d.align, writable: d.writable, relocs: Vec::new() };
        module.segments.push(segment);
        let (name, binding) = named(d.linkage, d.origin, &id.index());
        data.push(module.symbols.push(Sym { name, kind: Kind::Data(Some(id.index())), binding }));
    }
    let mut cx = Cx { unit, module, funcs, imports, data, passings, imported, stack_pointer };
    cx.relocate()?;
    cx.indirect()?;
    let bodies: Vec<(FuncId, &vun_lir::Function)> =
        unit.functions.iter().filter(|(id, _)| cx.funcs[*id].is_some()).collect();
    let lowered = bodies.par_map(|&(id, f)| {
        let _span = vun_trace::tracing::debug_span!("wasm", f = %f.linkage).entered();
        let mut lower = Lower::new(&cx, f, cx.passing(id))?;
        let shape = Shape::of(&mut lower);
        Emit::new(&mut lower, &shape).body(f.entry().expect("a function has a body"))?;
        Ok(lower.finish())
    });
    let funcs = lowered.into_iter().collect::<Result<Vec<Func>, Lack>>()?;
    let mut module = cx.module;
    module.funcs = funcs;
    Ok(module)
}

impl Cx<'_> {
    /// The addresses data holds, as its segments' relocations.
    fn relocate(&mut self) -> Result<(), Lack> {
        for (id, d) in self.unit.data.iter() {
            let mut relocs = Vec::new();
            for r in &d.relocs {
                let offset = u32::try_from(r.offset).expect("data of wasm32 fits 4 GiB");
                let addend = i32::try_from(r.addend).expect("addends of wasm32 fit i32");
                if matches!(r.target, Address::Func(_)) && addend != 0 {
                    return Err(Missing::FunctionOffset.at(d.origin));
                }
                relocs.push((offset, self.symbol(r.target), addend));
            }
            self.module.segments[id.index()].relocs = relocs;
        }
        Ok(())
    }

    /// Lists the types of the signatures indirect calls name, so bodies find them listed.
    fn indirect(&mut self) -> Result<(), Lack> {
        for (id, f) in self.unit.functions.iter() {
            if self.funcs[id].is_none() {
                continue;
            }
            for (_, inst) in f.insts.iter() {
                if let InstKind::Call { callee: Callee::Indirect(_, signature), .. } = &inst.kind {
                    let passing = Passing::of(signature).map_err(|m| m.at(inst.origin))?;
                    self.module.ty(passing.ty);
                }
            }
        }
        Ok(())
    }
}
