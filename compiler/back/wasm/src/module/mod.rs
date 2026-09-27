//! A relocatable WebAssembly object as data (the tool conventions' format): function types,
//! imported and defined functions, data segments, and the symbols relocations name. It is
//! checked by [`Module::verify`] and written by [`Module::write`].

mod code;
mod invalid;
mod link;
pub mod op;
mod text;
mod verify;
mod write;

pub use invalid::{Invalid, Why};
pub use text::Text;

use vun_hash::Map;
use vun_index::{IndexVec, index};
use vun_intern::Symbol;
use vun_origin::OriginId;

use op::Op;

index! {
    /// A function type, by its place in the type section.
    pub struct TypeId;
    /// A symbol, by its place in the symbol table relocations name.
    pub struct SymbolId;
}

/// A value type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ValType {
    I32,
    I64,
    F32,
    F64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct FuncType {
    pub params: Vec<ValType>,
    pub results: Vec<ValType>,
}

/// A defined function: its type, the locals past its parameters, its body up to the `end`
/// closing it, which the writer adds, and where the code of each instruction it was made from
/// starts in the body, with that instruction's origin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Func {
    pub ty: TypeId,
    pub locals: Vec<ValType>,
    pub body: Vec<Op>,
    pub marks: Vec<(usize, OriginId)>,
}

/// What a symbol names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A function of the type, from the import module of the library it comes from when
    /// another module defines it, or defined here (its body the [`Func`] at its place among the
    /// defined functions).
    Func { ty: TypeId, def: Def },
    /// Bytes of memory: a segment of this module, or defined elsewhere.
    Data(Option<usize>),
    /// What the linker defines for every object.
    Linker(Linker),
}

/// The import module of what another object or the linker defines.
pub const ENV: &str = "env";

/// What the linker defines for every object, imported from [`ENV`] under its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Linker {
    /// The global holding the top of the stack in linear memory, `__stack_pointer`.
    StackPointer,
    /// The table of the functions whose addresses are taken, `__indirect_function_table`.
    Table,
}

impl Linker {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::StackPointer => "__stack_pointer",
            Self::Table => "__indirect_function_table",
        }
    }
}

/// Where a function is defined.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Def {
    /// In another module, imported from `module`.
    Imported { module: Symbol },
    /// Here, the defined function at this place.
    Here(usize),
}

/// Who sees a symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Binding {
    /// This object only.
    Local,
    /// Every object, not the host.
    Hidden,
    /// Every object, and the host through the module's exports.
    Exported,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sym {
    pub name: Symbol,
    pub kind: Kind,
    pub binding: Binding,
}

/// Initialized memory, placed by the linker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    pub bytes: Vec<u8>,
    pub align: u64,
    pub writable: bool,
    /// The addresses written into the bytes: at an offset, of what a symbol names plus an addend.
    pub relocs: Vec<(u32, SymbolId, i32)>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Module {
    pub types: IndexVec<TypeId, FuncType>,
    pub funcs: Vec<Func>,
    pub segments: Vec<Segment>,
    pub symbols: IndexVec<SymbolId, Sym>,
    /// Each type's id, so a type is listed once.
    listed: Map<FuncType, TypeId>,
}

impl Module {
    /// The id of `ty`, listed once.
    pub fn ty(&mut self, ty: FuncType) -> TypeId {
        if let Some(&id) = self.listed.get(&ty) {
            return id;
        }
        let id = self.types.push(ty.clone());
        self.listed.insert(ty, id);
        id
    }

    /// The type's id, if it is listed.
    #[must_use]
    pub fn type_of(&self, ty: &FuncType) -> Option<TypeId> {
        self.listed.get(ty).copied()
    }

    /// The symbols of imported functions in the order of the function index space, then those of
    /// the defined ones: a function's index is its place in this order.
    #[must_use]
    pub fn functions(&self) -> Vec<SymbolId> {
        let imported = self
            .symbols
            .iter()
            .filter(|(_, s)| matches!(s.kind, Kind::Func { def: Def::Imported { .. }, .. }));
        let mut defined: Vec<_> = self
            .symbols
            .iter()
            .filter_map(|(id, s)| match s.kind {
                Kind::Func { def: Def::Here(at), .. } => Some((at, id)),
                _ => None,
            })
            .collect();
        defined.sort_unstable();
        imported.map(|(id, _)| id).chain(defined.into_iter().map(|(_, id)| id)).collect()
    }

    /// The symbol of what the linker defines, if the module names it.
    #[must_use]
    pub fn linker(&self, linker: Linker) -> Option<SymbolId> {
        self.symbols.iter().find(|(_, s)| s.kind == Kind::Linker(linker)).map(|(id, _)| id)
    }

    /// The symbol of what the linker defines, added when first named.
    pub fn linked(&mut self, linker: Linker) -> SymbolId {
        self.linker(linker).unwrap_or_else(|| {
            let name = Symbol::intern(linker.name());
            self.symbols.push(Sym { name, kind: Kind::Linker(linker), binding: Binding::Hidden })
        })
    }
}
