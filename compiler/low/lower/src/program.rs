//! Lowering a program (spec 08, section 1): each function its exports reach becomes a LIR function,
//! in the order `Programs` found them, so a function's id is its position; link symbols become
//! imports, and constants that live in memory become data objects, each made once.

use std::collections::HashMap;
use std::sync::Arc;

use vun_hir::db::FileRef;
use vun_index::Idx;
use vun_intern::Symbol;
use vun_lir::{Address, Data, DataId, FuncId, Import, ImportId, ImportKind, Linkage, Reloc, Unit};
use vun_mir::{Concrete, Programs};
use vun_origin::OriginId;
use vun_query::{Cx, Query};
use vun_sema::env::Env;
use vun_sema::eval::Instantiation;
use vun_sema::item::DeclRef;
use vun_sema::target::TargetOf;
use vun_sema::ty::Value;

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

/// What lowering a program keeps across its functions.
pub(crate) struct Lowering<'a, 'db> {
    pub(crate) types: Types<'a, 'db>,
    pub(crate) env: Env<'a, 'db>,
    pub(crate) unit: Unit,
    functions: HashMap<Instantiation, FuncId>,
    /// The `end` of each coroutine function, after every function; its `resume` is at the
    /// coroutine's own position.
    ends: HashMap<Instantiation, FuncId>,
    imports: HashMap<Symbol, ImportId>,
    /// The functions the program exports, by symbol: what a link symbol of the program names.
    exports: HashMap<Symbol, FuncId>,
    data: HashMap<(Vec<u8>, Vec<Reloc>, u64), DataId>,
    globals: HashMap<DeclRef, DataId>,
}

impl<'a, 'db> Lowering<'a, 'db> {
    fn program(cx: &'a Cx<'db>, root: &FileRef) -> Result<Unit, Unlowered> {
        let program = cx.get::<Programs>(root);
        let env = Env::of(cx, root).expect("the root file is read");
        // Reported at the root file's namespace, the program as a whole.
        let hir = &env.parsed.hir;
        let at = hir.namespaces[hir.root.expect("a lowered file has a root namespace")].origin;
        let target = cx.get::<TargetOf>(&()).ok_or(Unlowered { at, why: Why::Untargeted })?;
        let types = Types::new(cx, &target).map_err(|bits| Unlowered { at, why: Why::Address(bits) })?;
        let unit = Unit { pointer_bits: types.pointer_bits(), ..Unit::default() };
        let functions =
            program.functions.iter().enumerate().map(|(i, f)| (f.clone(), FuncId::new(i))).collect();
        let mut bodies = Vec::new();
        for f in &program.functions {
            bodies.push(cx.get::<Concrete>(f).map_err(|u| Unlowered { at: u.at(), why: Why::Unready(u) }));
        }
        let coroutines: Vec<&Instantiation> = program
            .functions
            .iter()
            .zip(&bodies)
            .filter(|(_, b)| b.as_ref().is_ok_and(|b| b.suspends()))
            .map(|(f, _)| f)
            .collect();
        let ends = coroutines
            .iter()
            .enumerate()
            .map(|(k, &f)| (f.clone(), FuncId::new(program.functions.len() + k)))
            .collect();
        let mut l = Self {
            types,
            env,
            unit,
            functions,
            ends,
            imports: HashMap::new(),
            exports: program.exports.iter().map(|&(i, symbol)| (symbol, FuncId::new(i))).collect(),
            data: HashMap::new(),
            globals: HashMap::new(),
        };
        if let Some((i, unready)) = program.unready.first() {
            return Err(Unlowered { at: i.f.literal.expr.origin, why: Why::Unready(unready.clone()) });
        }
        for (i, (f, body)) in program.functions.iter().zip(bodies).enumerate() {
            let body = body?;
            let export = program.exports.iter().find(|(at, _)| *at == i).map(|&(_, symbol)| symbol);
            let func = match export {
                None if body.suspends() => FnLower::coroutine(&mut l, f, &body, Mode::Resume)?,
                // A coroutine function's frames are built where it is called: it has no symbol.
                Some(_) if body.suspends() => return Err(Unlowered::unsupported(body.origin, Form::Frame)),
                export => {
                    FnLower::lower(&mut l, f, &body, export.map_or(Linkage::Internal, Linkage::Export))?
                }
            };
            l.unit.functions.push(func);
        }
        for f in coroutines {
            let body = cx.get::<Concrete>(f).map_err(|u| Unlowered { at: u.at(), why: Why::Unready(u) })?;
            let end = FnLower::coroutine(&mut l, f, &body, Mode::End)?;
            l.unit.functions.push(end);
        }
        Ok(l.unit)
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
        let id = self.unit.imports.push(Import { symbol, kind, origin: decl.decl.origin });
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
