//! HIR nodes, stored in struct-of-arrays arenas, one [`Hir`] per file.
//!
//! The HIR has the shape of the CST with roles decided: patterns are apart from expressions,
//! declarations name their binders, and `$for`/`$if` in a list are [`Entry::Staged`]. Every node
//! carries the origin of the source that produced it (spec 10, section 1).

mod binders;
mod enclosing;
mod expr;
mod index;
mod items;
mod pat;
mod path;
mod suspends;

use vun_index::{IndexVec, index};
use vun_intern::Symbol;
use vun_origin::OriginId;

pub use binders::each_entry;
pub use expr::{Arg, Cond, Expr, ExprKind, ForHead, Lit};
pub use items::{Arm, Entry, FieldDef, List, Member, Param, Spread, Staged, Stmt, Variant};
pub use pat::{Pat, PatKind};

index! {
    pub struct ExprId;
    pub struct PatId;
    pub struct BlockId;
    pub struct NsId;
    pub struct DeclId;
    pub struct BinderId;
}

/// One lowered file. Its root namespace holds the file's declarations.
#[derive(Default)]
pub struct Hir {
    pub root: Option<NsId>,
    pub namespaces: IndexVec<NsId, Namespace>,
    pub decls: IndexVec<DeclId, Decl>,
    pub exprs: IndexVec<ExprId, Expr>,
    pub pats: IndexVec<PatId, Pat>,
    pub blocks: IndexVec<BlockId, Block>,
    pub binders: IndexVec<BinderId, Binder>,
    /// What each node relates to, found once, when first asked.
    index: std::sync::OnceLock<index::Index>,
}

impl Hir {
    fn index(&self) -> &index::Index {
        self.index.get_or_init(|| index::Index::of(self))
    }
}

/// A name as written: literal, computed by `$(...)`, or the positional `_`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Name {
    Plain(Symbol),
    /// `$(e)`: a name produced at compile time (spec 03, section 2).
    Computed(ExprId),
    /// A tuple position: `t.0`.
    Index(u32),
    Hole,
}

/// As written, with a computed name as `$(...)`.
impl std::fmt::Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Plain(s) => s.fmt(f),
            Self::Computed(_) => f.write_str("$(...)"),
            Self::Index(i) => i.fmt(f),
            Self::Hole => f.write_str("_"),
        }
    }
}

/// Declarations whose order does not matter: a file or a `struct` body.
pub struct Namespace {
    pub members: List<Member>,
    pub origin: OriginId,
}

/// `pattern := value` or `pattern: type = value` in a namespace or a block.
pub struct Decl {
    pub pat: PatId,
    pub ty: Option<ExprId>,
    /// `None` for `mut buf: [N]u8;`, uninitialized.
    pub value: Option<ExprId>,
    pub attrs: Vec<ExprId>,
    pub doc: Vec<String>,
    pub origin: OriginId,
}

/// A name introduced by a pattern: a local, a parameter, or a namespace member.
pub struct Binder {
    pub name: Symbol,
    pub mutable: bool,
    /// `$name`: known at compile time.
    pub comptime: bool,
    pub origin: OriginId,
}

/// Statements run in order; the tail is the block's value.
pub struct Block {
    pub stmts: List<Stmt>,
    pub tail: Option<ExprId>,
    pub origin: OriginId,
}
