//! Assembling a program from its functions, each lowered on its own (`piece`) with its small
//! callees inlined (`inlined`): every id a function's LIR holds is placed in the program's unit
//! (`placing`). A declaration the linker provides is the function the program exports under its
//! symbol, else an import made once per symbol; constants are made once per content, a top-level
//! binding's object once per binding, and a trap site once per site, whichever functions hold
//! it, the ids that data and assembly blocks hold placed with it.

use vun_hash::Map;
use vun_index::Idx;
use vun_intern::Symbol;
use vun_lir::{
    Address, Data, DataId, FuncId, Import, ImportId, ImportKind, Linkage, Site, Trace, TraceLayout, Unit,
};
use vun_mir::{Concrete, Program};
use vun_origin::OriginId;
use vun_sema::eval::Instantiation;
use vun_sema::int::Integer;
use vun_sema::item::DeclRef;
use vun_sema::link::Linked;
use vun_sema::ty::{Ty, TyKind, Value};

use crate::lowering::Lowering;
use crate::piece::{Bytes, Datum, DatumKind, FuncRef, Lir, Piece};
use crate::placing::{Places, SiteIds};
use crate::unchecked::Unchecked;
use crate::unlowered::{Form, Unlowered, Why};

/// The program's unit being assembled, and where each thing named in it went.
pub(crate) struct Assembly<'l, 'a, 'db> {
    /// The lowering of the root file, where imports are typed and bytes written.
    l: &'l Lowering<'a, 'db>,
    unit: Unit,
    functions: Map<Instantiation, FuncId>,
    ends: Map<Instantiation, FuncId>,
    constructors: Map<(Ty, Symbol), FuncId>,
    /// The functions the program exports, by symbol: what a link symbol of the program names.
    exports: Map<Symbol, FuncId>,
    /// The symbol each function the program exports is exported under.
    exported: Map<FuncId, Symbol>,
    imports: Map<Symbol, ImportId>,
    constants: Map<(Bytes, u64), DataId>,
    globals: Map<DeclRef, DataId>,
    sites: SiteIds,
}

impl<'l, 'a, 'db> Assembly<'l, 'a, 'db> {
    /// The assembly of a program whose root file `l` lowers.
    pub(crate) fn new(l: &'l Lowering<'a, 'db>) -> Self {
        let unit = Unit { pointer_bits: l.types.pointer_bits(), checks: l.checks.clone(), ..Unit::default() };
        Self {
            l,
            unit,
            functions: Map::default(),
            ends: Map::default(),
            constructors: Map::default(),
            exports: Map::default(),
            exported: Map::default(),
            imports: Map::default(),
            constants: Map::default(),
            globals: Map::default(),
            sites: SiteIds::default(),
        }
    }

    /// The pieces of `program`, in the order of the unit's functions, each numbered: the
    /// functions reached, then each coroutine's `end`, then the constructors taken as values.
    /// The checks do not apply in `unchecked`; a constructor not of an enum is reported at `at`.
    pub(crate) fn number(
        &mut self,
        program: &Program,
        unchecked: &Unchecked,
        at: OriginId,
    ) -> Result<Vec<Piece>, Unlowered> {
        let cx = self.l.types.cx;
        let mut pieces: Vec<Piece> = Vec::new();
        for f in &program.functions {
            self.functions.insert(f.clone(), FuncId::new(pieces.len()));
            pieces.push(Piece::of(f, false, unchecked));
        }
        let suspends = |f: &&Instantiation| cx.get::<Concrete>(f).is_ok_and(|b| b.suspends());
        for f in program.functions.iter().filter(suspends) {
            self.ends.insert(f.clone(), FuncId::new(pieces.len()));
            pieces.push(Piece::of(f, true, unchecked));
        }
        for &(ty, name) in &program.constructors {
            let TyKind::Nominal(nominal) = &*ty else { return Err(Unlowered { at, why: Why::NoLayout(ty) }) };
            self.constructors.insert((ty, name), FuncId::new(pieces.len()));
            pieces.push(Piece::Constructor { nominal: nominal.clone(), name });
        }
        for &(i, symbol) in &program.exports {
            // A coroutine function's frames are built where it is called: it has no symbol.
            let f = &program.functions[i];
            if self.ends.contains_key(f) {
                return Err(Unlowered::unsupported(f.f.literal.expr.origin, Form::Frame));
            }
            self.exports.insert(symbol, FuncId::new(i));
            self.exported.insert(FuncId::new(i), symbol);
        }
        Ok(pieces)
    }

    /// Adds the function of `lir`, the next of the unit, under the symbol the program exports
    /// it as, if it does.
    pub(crate) fn add(&mut self, lir: &Lir, at: OriginId) -> Result<(), Unlowered> {
        let mut function = self.place(lir, at)?;
        if let Some(&symbol) = self.exported.get(&FuncId::new(self.unit.functions.len())) {
            function.linkage = Linkage::Export(symbol);
        }
        self.unit.functions.push(function);
        Ok(())
    }

    /// The unit, once every function is added, each with its small callees inlined already.
    pub(crate) fn finish(self) -> Unit {
        self.unit.verified();
        self.unit
    }
}

impl Places for Assembly<'_, '_, '_> {
    fn function(&mut self, named: &FuncRef, at: OriginId) -> Result<Address, Unlowered> {
        let placed = match named {
            FuncRef::Function(f) => self.functions.get(f),
            FuncRef::End(f) => self.ends.get(f),
            FuncRef::Constructor(ty, name) => self.constructors.get(&(*ty, *name)),
        };
        placed.map(|&f| Address::Func(f)).ok_or(Unlowered::unsupported(at, Form::Callee))
    }

    /// The function the program exports under the declaration's symbol, or an import, made once
    /// per symbol.
    fn linked(&mut self, decl: &DeclRef, at: OriginId) -> Result<Address, Unlowered> {
        let linked = decl.linked(self.l.types.cx).ok_or(Unlowered::unsupported(at, Form::Callee))?;
        if let Some(&f) = self.exports.get(&linked.symbol) {
            return Ok(Address::Func(f));
        }
        // The table's symbol, which `lang.trace` declares: the compiler defines it.
        let table =
            self.l.item(vun_items::lang::trace_symbols::table, at)?.text().map(|s| Symbol::intern(&s));
        if table == Some(linked.symbol) {
            return self.table(linked.symbol, at).map(Address::Data);
        }
        self.import(decl, linked, at).map(Address::Import)
    }

    /// The object of a top-level binding once per binding, and a constant once per content, its
    /// sites' ids written into its bytes.
    fn datum(&mut self, datum: Datum) -> DataId {
        if let DatumKind::Global { decl, .. } = &datum.kind
            && let Some(&id) = self.globals.get(decl)
        {
            return id;
        }
        let Datum { mut bytes, align, kind, origin } = datum;
        for site in std::mem::take(&mut bytes.sites) {
            self.l.int(&Integer::from(site.site as usize), site.width, &mut bytes.bytes, site.offset);
        }
        let key = (bytes, align);
        if let (DatumKind::Constant, Some(&id)) = (&kind, self.constants.get(&key)) {
            return id;
        }
        let writable = matches!(kind, DatumKind::Global { writable: true, .. });
        let (bytes, align) = key.clone();
        let data = Data {
            linkage: Linkage::Internal,
            bytes: bytes.bytes,
            relocs: bytes.relocs,
            align,
            writable,
            section: None,
            origin,
        };
        let id = self.unit.data.push(data);
        match kind {
            DatumKind::Constant => {
                self.constants.insert(key, id);
            }
            DatumKind::Global { decl, .. } => {
                self.globals.insert(decl, id);
            }
        }
        id
    }

    fn sites(&mut self, sites: &[Site], at: OriginId) -> Result<Vec<u32>, Unlowered> {
        sites.iter().map(|site| self.sites.id(&mut self.unit.sites, site, at)).collect()
    }
}

impl Assembly<'_, '_, '_> {
    /// The datum of the program's table (`lang.trace`), exported under `symbol`: made once, its
    /// bytes written when code is generated, with how `lang.trace` lays out its structs.
    fn table(&mut self, symbol: Symbol, at: OriginId) -> Result<DataId, Unlowered> {
        if let Some(trace) = &self.unit.trace {
            return Ok(trace.table);
        }
        let fields = |item| match self.l.item(item, at)? {
            Value::Type(ty) => self.l.types.fields(ty, at),
            _ => Err(Unlowered::unsupported(at, Form::Constant)),
        };
        let layout = TraceLayout {
            table: fields(vun_items::lang::trace::Table)?,
            function: fields(vun_items::lang::trace::Function)?,
            site: fields(vun_items::lang::trace::Site)?,
            origin: fields(vun_items::lang::trace::Origin)?,
            text: fields(vun_items::lang::trace::Text)?,
        };
        let table = self.unit.data.push(Data {
            linkage: Linkage::Export(symbol),
            bytes: vec![0; usize::try_from(layout.table.size).unwrap_or_default()],
            relocs: Vec::new(),
            align: layout.table.align,
            writable: false,
            section: None,
            origin: at,
        });
        let detail = self.l.types.cx.get::<vun_sema::trace::TraceOf>(&());
        self.unit.trace = Some(Trace { table, layout, detail });
        Ok(table)
    }

    fn import(&mut self, decl: &DeclRef, linked: Linked, at: OriginId) -> Result<ImportId, Unlowered> {
        let Linked { symbol, abi, library } = linked;
        if let Some(&id) = self.imports.get(&symbol) {
            return Ok(id);
        }
        let ty = self
            .l
            .types
            .cx
            .get::<vun_sema::decl::DeclType>(decl)
            .map_err(|_| Unlowered::unsupported(at, Form::Callee))?
            .ty;
        let kind = match ty.signature() {
            Some(signature) => ImportKind::Func(self.l.signature_of(&signature, abi, at)?),
            None => ImportKind::Data,
        };
        // The largest bound the program's declarations of the symbol state, the handler of
        // traps included, which a program declares itself (decision 164).
        let stack_max = self.l.env.linked_stack(symbol);
        let id =
            self.unit.imports.push(Import { symbol, library, kind, origin: decl.decl.origin, stack_max });
        self.imports.insert(symbol, id);
        Ok(id)
    }
}
