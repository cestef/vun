//! HIR nodes, stored in struct-of-arrays arenas, one [`Hir`] per file.
//!
//! The HIR has the shape of the CST with roles decided: patterns are apart from expressions,
//! declarations name their binders, and `$for`/`$if` in a list are [`Entry::Staged`]. Every node
//! carries the origin of the source that produced it (spec 10, section 1).

mod binders;
mod expr;
mod items;
mod pat;
mod path;

use overt_index::{IndexVec, index};
use overt_intern::Symbol;
use overt_origin::OriginId;

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
