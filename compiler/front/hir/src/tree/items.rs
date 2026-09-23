//! Lists and the entries of namespaces, blocks, calls and matches.

use overt_grammar::TokenKind;
use overt_origin::OriginId;

use super::{BinderId, BlockId, DeclId, ExprId, Name, PatId};

/// A list whose entries may be generated at compile time.
pub type List<T> = Vec<Entry<T>>;

pub enum Entry<T> {
    One(T),
    /// `$for` or `$if` producing entries of the same list (spec 03, section 2).
    Staged(Staged<T>),
}

pub enum Staged<T> {
    For { pat: PatId, iter: ExprId, body: List<T>, origin: OriginId },
    If { cond: ExprId, then: List<T>, otherwise: List<T>, origin: OriginId },
}

pub enum Member {
    Decl(DeclId),
    Field(FieldDef),
    Spread(Spread),
}

/// `..path` or `..path.{ a, b }`: members of the namespace at `path` become members here.
pub struct Spread {
    pub path: ExprId,
    /// The selected names, binders of the enclosing namespace; `None` spreads every public member.
    pub select: Option<Vec<BinderId>>,
    pub origin: OriginId,
}

/// `name: T = default` in a `struct`.
pub struct FieldDef {
    pub name: Name,
    pub mutable: bool,
    pub ty: ExprId,
    pub default: Option<ExprId>,
    pub attrs: Vec<ExprId>,
    pub doc: Vec<String>,
    pub origin: OriginId,
}

pub struct Variant {
    pub name: Name,
    pub payload: Vec<ExprId>,
    pub doc: Vec<String>,
    pub origin: OriginId,
}

pub enum Stmt {
    Decl(DeclId),
    Expr(ExprId),
    /// `place op value`, `op` one of the assignment tokens.
    Assign {
        op: TokenKind,
        place: ExprId,
        value: ExprId,
        origin: OriginId,
    },
    /// A nested block of statements, such as the body of a statement-level `$for`.
    Block(BlockId),
}

/// A parameter of a function literal or a function type.
pub struct Param {
    /// `None` in a function type written without names: `(u8, bool) -> u8`.
    pub pat: Option<PatId>,
    pub ty: Option<ExprId>,
    pub attrs: Vec<ExprId>,
    pub origin: OriginId,
}

pub struct Arm {
    pub pat: PatId,
    pub guard: Option<ExprId>,
    pub value: Option<ExprId>,
    pub origin: OriginId,
}
