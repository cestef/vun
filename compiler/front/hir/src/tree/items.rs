//! Lists and the entries of namespaces, blocks, calls and matches.

use vun_grammar::TokenKind;
use vun_origin::OriginId;

use super::{BinderId, BlockId, Cond, DeclId, ExprId, Name, NsId, PatId};

/// A list whose entries may be generated at compile time.
pub type List<T> = Vec<Entry<T>>;

#[derive(vun_codec::Encode, vun_codec::Decode)]
pub enum Entry<T> {
    One(T),
    /// `$for` or `$if` producing entries of the same list (spec 03, section 2).
    Staged(Staged<T>),
}

impl<T> Entry<T> {
    /// The item, for an entry written as one; `None` for one generated at compile time.
    pub fn written(&self) -> Option<&T> {
        match self {
            Self::One(item) => Some(item),
            Self::Staged(_) => None,
        }
    }
}

#[derive(vun_codec::Encode, vun_codec::Decode)]
pub enum Staged<T> {
    For {
        pat: PatId,
        iter: ExprId,
        body: List<T>,
        origin: OriginId,
    },
    /// `$if c { then } else { otherwise }`; a test `$if p = e` binds what `p` takes in `then`.
    If {
        cond: Cond,
        then: List<T>,
        otherwise: List<T>,
        origin: OriginId,
    },
}

#[derive(vun_codec::Encode, vun_codec::Decode)]
pub enum Member {
    Decl(DeclId),
    Field(FieldDef),
    Spread(Spread),
}

/// `..path` or `..path.{ a, b }`: members of the namespace at `path` become members here.
#[derive(vun_codec::Encode, vun_codec::Decode)]
pub struct Spread {
    pub path: ExprId,
    /// The selected names, binders of the enclosing namespace; `None` spreads every public member.
    pub select: Option<Vec<BinderId>>,
    pub origin: OriginId,
}

/// `name: T = default` in a `struct`.
#[derive(vun_codec::Encode, vun_codec::Decode)]
pub struct FieldDef {
    pub name: Name,
    pub mutable: bool,
    pub ty: ExprId,
    pub default: Option<ExprId>,
    pub attrs: Vec<ExprId>,
    pub doc: Vec<String>,
    pub origin: OriginId,
}

#[derive(vun_codec::Encode, vun_codec::Decode)]
pub struct Variant {
    pub name: Name,
    pub payload: Vec<ExprId>,
    /// The integer its tag is, when written `name = code`.
    pub code: Option<ExprId>,
    pub attrs: Vec<ExprId>,
    pub doc: Vec<String>,
    pub origin: OriginId,
}

#[derive(vun_codec::Encode, vun_codec::Decode)]
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
    /// `..path;` in a block: a namespace holding the spread alone, entered where it is written
    /// and left at the block's end, so that the names it brings are found after every scope
    /// (spec 01, section 8).
    Spread(NsId),
}

/// A parameter of a function literal or a function type.
#[derive(vun_codec::Encode, vun_codec::Decode)]
pub struct Param {
    /// `None` in a function type written without names: `(u8, bool) -> u8`.
    pub pat: Option<PatId>,
    pub ty: Option<ExprId>,
    pub attrs: Vec<ExprId>,
    pub origin: OriginId,
}

#[derive(vun_codec::Encode, vun_codec::Decode)]
pub struct Arm {
    pub pat: PatId,
    pub guard: Option<ExprId>,
    pub value: Option<ExprId>,
    pub origin: OriginId,
}
