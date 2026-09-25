//! Lowering a program (spec 08, section 1): each function its exports reach becomes a LIR function,
//! in the order `Programs` found them, so a function's id is its position; link symbols become
//! imports, and constants that live in memory become data objects, each made once.

use std::collections::HashMap;
use std::sync::Arc;

use vun_hir::db::FileRef;
use vun_index::Idx;
use vun_intern::Symbol;
use vun_lir::{Address, Data, DataId, FuncId, Function, Import, ImportId, ImportKind, Linkage, Reloc, Unit};
use vun_mir::{Body, Concrete, Program, Programs, Roots};
use vun_origin::OriginId;
use vun_query::{Cx, Query};
use vun_sema::env::Env;
use vun_sema::eval::Instantiation;
use vun_sema::item::DeclRef;
use vun_sema::target::TargetOf;
use vun_sema::ty::{Ty, TyKind, Value};

use crate::func::{FnLower, Mode};
use crate::repr::Types;
use crate::unlowered::{Form, Unlowered, Why};

/// The LIR of the program whose root is a file.
pub struct Lowered;

impl Query for Lowered {
    const NAME: &'static str = "lowered";
    type Key = FileRef;
    type Value = Arc<Result<Unit, Unlowered>>;

    fn compute(cx: &Cx<'_>, root: &FileRef) -> Self::Value {
        Arc::new(Lowering::program(cx, root))
    }
}

/// Each function a file's functions reach (`Roots::Functions`), lowered on its own, and why it
/// does not lower if it does not: what coverage measures.
pub struct EachLowered;

impl Query for EachLowered {
    const NAME: &'static str = "each_lowered";
    type Key = FileRef;
    type Value = Arc<Vec<(Instantiation, Result<(), Unlowered>)>>;

    fn compute(cx: &Cx<'_>, root: &FileRef) -> Self::Value {
        Arc::new(Lowering::each(cx, root))
    }
}

/// What lowering a program keeps across its functions.
pub(crate) struct Lowering<'a, 'db> {
    pub(crate) types: Types<'a, 'db>,
    pub(crate) env: Env<'a, 'db>,
    pub(crate) unit: Unit,
    functions: HashMap<Instantiation, FuncId>,
    /// The `end` of each coroutine function, after every function; its `resume` is at the
    /// coroutine's own position.
    ends: HashMap<Instantiation, FuncId>,
    /// The function of each variant's constructor taken as a value, after the `end`s.
    constructors: HashMap<(Ty, Symbol), FuncId>,
    imports: HashMap<Symbol, ImportId>,
    /// The functions the program exports, by symbol: what a link symbol of the program names.
    exports: HashMap<Symbol, FuncId>,
    data: HashMap<(Vec<u8>, Vec<Reloc>, u64), DataId>,
    globals: HashMap<DeclRef, DataId>,
}

impl<'a, 'db> Lowering<'a, 'db> {
    fn program(cx: &'a Cx<'db>, root: &FileRef) -> Result<Unit, Unlowered> {
        let program = cx.get::<Programs>(&(root.clone(), Roots::Exports));
        let mut l = Self::new(cx, root, &program)?;
        if let Some((i, unready)) = program.unready.first() {
            return Err(Unlowered { at: i.f.literal.expr.origin, why: Why::Unready(unready.clone()) });
        }
        let mut ends = Vec::new();
        for f in 0..program.functions.len() {
            let (resume, end) = l.lower_at(&program, f)?;
            l.unit.functions.push(resume);
            ends.extend(end);
        }
        for end in ends {
            l.unit.functions.push(end);
        }
        for &(ty, name) in &program.constructors {
            let constructor = l.lower_constructor(ty, name)?;
            l.unit.functions.push(constructor);
        }
        Ok(l.unit)
    }

    /// Each function the root file's functions reach, and why it does not lower if it does not.
    fn each(cx: &'a Cx<'db>, root: &FileRef) -> Vec<(Instantiation, Result<(), Unlowered>)> {
        let program = cx.get::<Programs>(&(root.clone(), Roots::Functions));
        let mut l = Self::new(cx, root, &program);
        let functions = program.functions.iter().enumerate();
        functions
            .map(|(i, f)| {
                let lowered = match &mut l {
                    Ok(l) => l.lower_at(&program, i).map(drop),
                    Err(unlowered) => Err(unlowered.clone()),
                };
                (f.clone(), lowered)
            })
            .collect()
    }

    /// The lowering of `program`: each function's id is its position, and each coroutine's
    /// `end` comes after every function.
    fn new(cx: &'a Cx<'db>, root: &FileRef, program: &Program) -> Result<Self, Unlowered> {
        let env = Env::of(cx, root).expect("the root file is read");
        let at = Self::root_of(&env);
        let target = cx.get::<TargetOf>(&()).ok_or(Unlowered { at, why: Why::Untargeted })?;
        let types = Types::new(cx, &target).map_err(|bits| Unlowered { at, why: Why::Address(bits) })?;
        let unit = Unit { pointer_bits: types.pointer_bits(), ..Unit::default() };
        let functions =
            program.functions.iter().enumerate().map(|(i, f)| (f.clone(), FuncId::new(i))).collect();
        let suspends = |f: &&Instantiation| cx.get::<Concrete>(f).is_ok_and(|b| b.suspends());
        let coroutines = program.functions.iter().filter(suspends);
        let ends: HashMap<Instantiation, FuncId> = coroutines
            .enumerate()
            .map(|(k, f)| (f.clone(), FuncId::new(program.functions.len() + k)))
            .collect();
        let first = program.functions.len() + ends.len();
        let constructors =
            program.constructors.iter().enumerate().map(|(k, &c)| (c, FuncId::new(first + k))).collect();
        Ok(Self {
            types,
            env,
            unit,
            functions,
            ends,
            constructors,
            imports: HashMap::new(),
            exports: program.exports.iter().map(|&(i, symbol)| (symbol, FuncId::new(i))).collect(),
            data: HashMap::new(),
            globals: HashMap::new(),
        })
    }

    /// Where the program as a whole is reported: the root file's namespace.
    fn root_of(env: &Env<'_, '_>) -> OriginId {
        let hir = &env.parsed.hir;
        hir.namespaces[hir.root.expect("a lowered file has a root namespace")].origin
    }

    fn root(&self) -> OriginId {
        Self::root_of(&self.env)
    }

    /// The function the constructor of the variant `name` of `ty` is as a value.
    fn lower_constructor(&mut self, ty: Ty, name: Symbol) -> Result<Function, Unlowered> {
        let cx = self.types.cx;
        let TyKind::Nominal(nominal) = &*ty else {
            return Err(Unlowered { at: self.root(), why: Why::NoLayout(ty) });
        };
        let at = nominal.literal.expr.origin;
        let body = Body::constructor(cx, ty, name).ok_or(Unlowered { at, why: Why::NoLayout(ty) })?;
        let params = body.params.clone();
        FnLower::function(self, &body, &params, None, Linkage::Internal, Arc::default())
    }

    /// The id of the function the constructor of the variant `name` of `ty` is as a value.
    pub(crate) fn constructor(&self, ty: Ty, name: Symbol, at: OriginId) -> Result<FuncId, Unlowered> {
        self.constructors.get(&(ty, name)).copied().ok_or(Unlowered::unsupported(at, Form::Callee))
    }

    /// The function at position `i` of `program`, and its `end` when it is a coroutine.
    fn lower_at(&mut self, program: &Program, i: usize) -> Result<(Function, Option<Function>), Unlowered> {
        let f = &program.functions[i];
        let body =
            self.types.cx.get::<Concrete>(f).map_err(|u| Unlowered { at: u.at(), why: Why::Unready(u) })?;
        let export = program.exports.iter().find(|(at, _)| *at == i).map(|&(_, symbol)| symbol);
        let resume = match export {
            None if body.suspends() => FnLower::coroutine(self, f, &body, Mode::Resume)?,
            // A coroutine function's frames are built where it is called: it has no symbol.
            Some(_) if body.suspends() => return Err(Unlowered::unsupported(body.origin, Form::Frame)),
            export => FnLower::lower(self, f, &body, export.map_or(Linkage::Internal, Linkage::Export))?,
        };
        let end = if body.suspends() { Some(FnLower::coroutine(self, f, &body, Mode::End)?) } else { None };
        Ok((resume, end))
    }

    /// The id of the function `f` lowers to.
    pub(crate) fn function(&self, f: &Instantiation, at: OriginId) -> Result<FuncId, Unlowered> {
        self.functions.get(f).copied().ok_or(Unlowered::unsupported(at, Form::Callee))
    }

    /// The `resume` and `end` of the coroutine instantiation `f`.
    pub(crate) fn coroutine(&self, f: &Instantiation, at: OriginId) -> Result<(FuncId, FuncId), Unlowered> {
        let unlowered = || Unlowered::unsupported(at, Form::Frame);
        Ok((*self.functions.get(f).ok_or_else(unlowered)?, *self.ends.get(f).ok_or_else(unlowered)?))
    }

    /// What the declaration the linker provides names: the function the program exports under
    /// its symbol, or else an import, made once per symbol.
    pub(crate) fn linked(&mut self, decl: &DeclRef, at: OriginId) -> Result<Address, Unlowered> {
        let cx = self.types.cx;
        let (symbol, abi) = decl.linked(cx).ok_or(Unlowered::unsupported(at, Form::Callee))?;
        if let Some(&f) = self.exports.get(&symbol) {
            return Ok(Address::Func(f));
        }
        self.import(decl, symbol, abi, at).map(Address::Import)
    }

    fn import(
        &mut self,
        decl: &DeclRef,
        symbol: Symbol,
        abi: Option<vun_sema::primitives::abi::Abi>,
        at: OriginId,
    ) -> Result<ImportId, Unlowered> {
        let cx = self.types.cx;
        if let Some(&id) = self.imports.get(&symbol) {
            return Ok(id);
        }
        let ty = cx
            .get::<vun_sema::decl::DeclType>(decl)
            .map_err(|_| Unlowered::unsupported(at, Form::Callee))?
            .ty;
        let kind = match ty.signature() {
            Some(signature) => ImportKind::Func(self.signature_of(&signature, abi, at)?),
            None => ImportKind::Data,
        };
        // The largest bound the program's declarations of the symbol state, the handler of
        // traps included, which a program declares itself (decision 164).
        let stack_max = self.env.linked_stack(symbol);
        let id = self.unit.imports.push(Import { symbol, kind, origin: decl.decl.origin, stack_max });
        self.imports.insert(symbol, id);
        Ok(id)
    }

    /// A read-only data object holding `bytes`, made once per content.
    pub(crate) fn constant(
        &mut self,
        bytes: Vec<u8>,
        relocs: Vec<Reloc>,
        align: u64,
        origin: OriginId,
    ) -> DataId {
        let key = (bytes, relocs, align);
        if let Some(&id) = self.data.get(&key) {
            return id;
        }
        let (bytes, relocs, align) = key.clone();
        let data =
            Data { linkage: Linkage::Internal, bytes, relocs, align, writable: false, section: None, origin };
        let id = self.unit.data.push(data);
        self.data.insert(key, id);
        id
    }

    /// The data object holding the top-level binding `decl`, of type `ty`, from its initial
    /// value; `writable` for a `mut` one. Made once per binding.
    pub(crate) fn global(
        &mut self,
        decl: &DeclRef,
        ty: vun_sema::ty::Ty,
        writable: bool,
        at: OriginId,
    ) -> Result<DataId, Unlowered> {
        if let Some(&id) = self.globals.get(decl) {
            return Ok(id);
        }
        let value = self
            .types
            .cx
            .get::<vun_sema::decl::DeclValue>(decl)
            .map_err(|_| Unlowered::unsupported(at, Form::Constant))?;
        let bytes = self.encode(&value, ty, at)?;
        let align = self.types.shape(ty, at)?.layout.align;
        let data = Data {
            linkage: Linkage::Internal,
            bytes: bytes.bytes,
            relocs: bytes.relocs,
            align,
            writable,
            section: None,
            origin: decl.decl.origin,
        };
        let id = self.unit.data.push(data);
        self.globals.insert(decl.clone(), id);
        Ok(id)
    }

    /// The value of the `lang` item `item`.
    pub(crate) fn item(&self, item: vun_items::Item, at: OriginId) -> Result<Value, Unlowered> {
        self.env.item(item).map_err(|_| Unlowered::unsupported(at, Form::Constant))
    }
}
