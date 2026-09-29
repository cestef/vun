//! What lowering one function keeps: the target's types, the environment of the function's file,
//! and what the function names, each numbered in the order it is first named. The ids of the
//! function's LIR are positions in its own tables (`piece::Refs`), placed in the program's unit
//! when it is assembled (`assemble`), so a function lowers alike in every program reaching it.

use vun_hash::Map;
use vun_hir::db::FileRef;
use vun_index::Idx;
use vun_intern::Symbol;
use vun_lir::{Address, DataId, FuncId, ImportId, Site};
use vun_origin::OriginId;
use vun_query::Cx;
use vun_sema::env::Env;
use vun_sema::link::Linkable;
use vun_sema::target::TargetOf;
use vun_sema::thread::PerThread;
use vun_sema_ty::call::Instantiation;
use vun_sema_ty::item::DeclRef;
use vun_sema_ty::{Ty, Value};
use vun_target::{Checks, Threads};

use crate::piece::{Bytes, Datum, DatumKind, FuncRef, Refs};
use crate::repr::Types;
use crate::unlowered::{Form, Unlowered, Why};

/// What lowering one function keeps across it.
pub(crate) struct Lowering<'a, 'db> {
    pub(crate) types: Types<'a, 'db>,
    pub(crate) env: Env<'a, 'db>,
    /// The checks of unsafe code the program is built with.
    pub(crate) checks: Checks,
    /// How many threads the target's programs run, which storage per thread follows.
    pub(crate) threads: Threads,
    /// Whether the checks apply in the function lowered, which is not one they run in.
    pub(crate) checked: bool,
    /// What the function names, by the positions its ids are.
    pub(crate) refs: Refs,
    functions: Map<FuncRef, FuncId>,
    imports: Map<Symbol, ImportId>,
    constants: Map<(Bytes, u64), DataId>,
    globals: Map<DeclRef, DataId>,
}

impl<'a, 'db> Lowering<'a, 'db> {
    /// The lowering of a function of `file`, for the target chosen.
    pub(crate) fn new(cx: &'a Cx<'db>, file: &FileRef, checked: bool) -> Result<Self, Unlowered> {
        let env = Env::of(cx, file).expect("a lowered function's file is read");
        let at = Self::root_of(&env);
        let target = cx.get::<TargetOf>(&()).ok_or(Unlowered { at, why: Why::Untargeted })?;
        let types = Types::new(cx, &target).map_err(|bits| Unlowered { at, why: Why::Address(bits) })?;
        let checks = cx.get::<vun_sema::checks::ChecksOf>(&());
        Ok(Self {
            types,
            env,
            checks,
            threads: target.threads,
            checked,
            refs: Refs::default(),
            functions: Map::default(),
            imports: Map::default(),
            constants: Map::default(),
            globals: Map::default(),
        })
    }

    /// Where a file as a whole is reported: its namespace.
    pub(crate) fn root_of(env: &Env<'_, '_>) -> OriginId {
        let hir = &env.parsed.hir;
        hir.namespaces[hir.root.expect("a lowered file has a root namespace")].origin
    }

    /// The id of the function `named`, given the first time it is named.
    fn named(&mut self, named: FuncRef) -> FuncId {
        let functions = &mut self.refs.functions;
        *self.functions.entry(named).or_insert_with_key(|named| {
            functions.push(named.clone());
            FuncId::new(functions.len() - 1)
        })
    }

    /// The id of the function `f` lowers to.
    pub(crate) fn function(&mut self, f: &Instantiation) -> FuncId {
        self.named(FuncRef::Function(f.clone()))
    }

    /// The `resume` and `end` of the coroutine instantiation `f`.
    pub(crate) fn coroutine(&mut self, f: &Instantiation) -> (FuncId, FuncId) {
        (self.named(FuncRef::Function(f.clone())), self.named(FuncRef::End(f.clone())))
    }

    /// The id of the function the constructor of the variant `name` of `ty` is as a value.
    pub(crate) fn constructor(&mut self, ty: Ty, name: Symbol) -> FuncId {
        self.named(FuncRef::Constructor(ty, name))
    }

    /// What the declaration the linker provides names: an import here, which the program makes
    /// the function it exports under the symbol, or its own import (`assemble`).
    pub(crate) fn linked(&mut self, decl: &DeclRef, at: OriginId) -> Result<Address, Unlowered> {
        let linked = decl.linked(self.types.cx).ok_or(Unlowered::unsupported(at, Form::Callee))?;
        let imports = &mut self.refs.imports;
        let id = *self.imports.entry(linked.symbol).or_insert_with(|| {
            imports.push(decl.clone());
            ImportId::new(imports.len() - 1)
        });
        Ok(Address::Import(id))
    }

    /// The id here of a new trap site, `None` past what an id holds.
    pub(crate) fn site(&mut self, site: Site) -> Option<u32> {
        let id = u32::try_from(self.refs.sites.len()).ok()?;
        self.refs.sites.push(site);
        Some(id)
    }

    /// A read-only data object holding `bytes`, made once per content.
    pub(crate) fn constant(&mut self, bytes: Bytes, align: u64, origin: OriginId) -> DataId {
        let key = (bytes, align);
        if let Some(&id) = self.constants.get(&key) {
            return id;
        }
        let id = self.datum(Datum { bytes: key.0.clone(), align, kind: DatumKind::Constant, origin });
        self.constants.insert(key, id);
        id
    }

    /// The data object holding the top-level binding `decl`, of type `ty`, from its initial
    /// value; `writable` for a `mut` one. Made once per binding, here and in the program.
    pub(crate) fn global(
        &mut self,
        decl: &DeclRef,
        ty: Ty,
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
        let kind = DatumKind::Global { decl: decl.clone(), writable };
        let id = self.datum(Datum { bytes, align, kind, origin: decl.decl.origin });
        self.globals.insert(decl.clone(), id);
        Ok(id)
    }

    /// A new data object: after those its bytes point to, which encoding it made first.
    fn datum(&mut self, datum: Datum) -> DataId {
        self.refs.data.push(datum);
        DataId::new(self.refs.data.len() - 1)
    }

    /// Whether the top-level binding `decl` has one storage per thread here: its type says so,
    /// and the target runs threads.
    pub(crate) fn per_thread(&self, decl: &DeclRef) -> bool {
        self.threads == Threads::Many && decl.per_thread(self.types.cx)
    }

    /// The value of the `lang` item `item`.
    pub(crate) fn item(&self, item: vun_items::Item, at: OriginId) -> Result<Value, Unlowered> {
        self.env.item(item).map_err(|_| Unlowered::unsupported(at, Form::Constant))
    }
}
