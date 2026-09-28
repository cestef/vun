//! Assembling a program from its functions, each lowered on its own (`piece`): every id a
//! function's LIR holds is placed in the program's unit. A declaration the linker provides is
//! the function the program exports under its symbol, else an import made once per symbol;
//! constants are made once per content and a top-level binding's object once per binding; trap
//! sites are numbered in the order the functions come, the ids their data and assembly blocks
//! hold moved past the sites of the functions before.

use vun_hash::{Map, Set};
use vun_index::Idx;
use vun_intern::Symbol;
use vun_lir::{
    Address, Data, DataId, FuncId, Function, Import, ImportId, ImportKind, InstKind, Linkage, Unit,
};
use vun_lir_passes::Simplify;
use vun_mir::{Concrete, Program};
use vun_origin::OriginId;
use vun_sema::eval::Instantiation;
use vun_sema::int::Integer;
use vun_sema::item::DeclRef;
use vun_sema::link::Linked;
use vun_sema::ty::{Ty, TyKind};

use crate::lowering::Lowering;
use crate::piece::{Bytes, Datum, DatumKind, FuncRef, Lir, Piece};
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
}

/// Where the ids of one function's LIR go in the unit.
struct Placing {
    functions: Vec<Address>,
    imports: Vec<Address>,
    data: Vec<DataId>,
    /// The id of the function's first trap site.
    base: u32,
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
        }
    }

    /// The pieces of `program`, in the order of the unit's functions, each numbered: the
    /// functions reached, then each coroutine's `end`, then the constructors taken as values.
    /// The checks do not apply in `unchecked`; a constructor not of an enum is reported at `at`.
    pub(crate) fn number(
        &mut self,
        program: &Program,
        unchecked: &Set<Instantiation>,
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

    /// The unit, once every function is added: callees first, each inlines its small callees.
    pub(crate) fn finish(mut self) -> Unit {
        self.unit.inline_small();
        self.unit
    }

    /// The function of `lir`, its ids placed in the unit, reported at `at` when one is not.
    fn place(&mut self, lir: &Lir, at: OriginId) -> Result<Function, Unlowered> {
        let unplaced = || Unlowered::unsupported(at, Form::Callee);
        let base = u32::try_from(self.unit.sites.len()).map_err(|_| unplaced())?;
        self.unit.sites.extend(lir.refs.sites.iter().cloned());
        let functions =
            lir.refs.functions.iter().map(|f| self.function(f).map(Address::Func).ok_or_else(unplaced));
        let functions = functions.collect::<Result<_, _>>()?;
        let imports = lir.refs.imports.iter().map(|decl| self.linked(decl, at)).collect::<Result<_, _>>()?;
        let mut placing = Placing { functions, imports, data: Vec::with_capacity(lir.refs.data.len()), base };
        for datum in &lir.refs.data {
            let id = self.datum(datum, &placing);
            placing.data.push(id);
        }
        let mut function = lir.function.clone();
        function.map_addresses(|a| placing.address(a));
        for (_, inst) in function.insts.iter_mut() {
            if let InstKind::Asm(_, Some(check)) = &mut inst.kind {
                check.site += base;
            }
        }
        Ok(function)
    }

    /// The id in the unit of the function `named`.
    fn function(&self, named: &FuncRef) -> Option<FuncId> {
        match named {
            FuncRef::Function(f) => self.functions.get(f).copied(),
            FuncRef::End(f) => self.ends.get(f).copied(),
            FuncRef::Constructor(ty, name) => self.constructors.get(&(*ty, *name)).copied(),
        }
    }

    /// The id in the unit of `datum`, its addresses and sites placed as `placing` says.
    fn datum(&mut self, datum: &Datum, placing: &Placing) -> DataId {
        if let DatumKind::Global { decl, .. } = &datum.kind
            && let Some(&id) = self.globals.get(decl)
        {
            return id;
        }
        let mut bytes = datum.bytes.clone();
        for reloc in &mut bytes.relocs {
            reloc.target = placing.address(reloc.target);
        }
        for site in std::mem::take(&mut bytes.sites) {
            let id = Integer::from((placing.base + site.site) as usize);
            self.l.int(&id, site.width, &mut bytes.bytes, site.offset);
        }
        let key = (bytes, datum.align);
        if let (DatumKind::Constant, Some(&id)) = (&datum.kind, self.constants.get(&key)) {
            return id;
        }
        let writable = matches!(datum.kind, DatumKind::Global { writable: true, .. });
        let (bytes, align) = key.clone();
        let data = Data {
            linkage: Linkage::Internal,
            bytes: bytes.bytes,
            relocs: bytes.relocs,
            align,
            writable,
            section: None,
            origin: datum.origin,
        };
        let id = self.unit.data.push(data);
        match &datum.kind {
            DatumKind::Constant => {
                self.constants.insert(key, id);
            }
            DatumKind::Global { decl, .. } => {
                self.globals.insert(decl.clone(), id);
            }
        }
        id
    }

    /// What the declaration the linker provides names in the program: the function it exports
    /// under its symbol, or an import, made once per symbol.
    fn linked(&mut self, decl: &DeclRef, at: OriginId) -> Result<Address, Unlowered> {
        let linked = decl.linked(self.l.types.cx).ok_or(Unlowered::unsupported(at, Form::Callee))?;
        if let Some(&f) = self.exports.get(&linked.symbol) {
            return Ok(Address::Func(f));
        }
        self.import(decl, linked, at).map(Address::Import)
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

impl Placing {
    /// Where `address`, of the function's own ids, is in the unit.
    fn address(&self, address: Address) -> Address {
        match address {
            Address::Func(f) => self.functions[f.index()],
            Address::Import(i) => self.imports[i.index()],
            Address::Data(d) => Address::Data(self.data[d.index()]),
        }
    }
}
