//! The forms of made source: one [`Expr`] for values, types and patterns, whose variants are the
//! kinds of nodes of the grammar's productions (`nodes.vun`).

use vun_grammar::TokenKind;
use vun_items::Item as Decl;

use super::{Arm, Bind, Block, Borrow, Function, Init, Integer, Member, Param, Property, Variant};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    Int(Integer),
    /// The bits of an `f64` (`Expr::float`), so that expressions stay comparable.
    Float(u64),
    Char(char),
    String(String),
    /// `c"text"`, a string for C.
    CString(String),
    /// `_`: provided from outside (spec 01, section 3), or a pattern discarding.
    Hole,
    /// `()`
    Unit,
    /// `a.b.c`
    Path(Vec<String>),
    /// `(a, b)`
    Tuple(Vec<Expr>),
    /// `[a, b]`
    List(Vec<Expr>),
    /// `[value; count]`
    Repeat(Box<Expr>, Box<Expr>),
    /// `.{ .a = x }`, or `.{ x, y }` for a tuple
    Record(Vec<Init>),
    /// `T{ .a = x }`
    Typed(Box<Expr>, Vec<Init>),
    /// `{ statements; tail }`
    Block(Block),
    /// `[len]item`, the type of an array.
    ArrayType(Box<Expr>, Box<Expr>),
    /// `(params) -> result`, a function type; `None` for the result `()`.
    FnType(Vec<Param>, Option<Box<Expr>>),
    /// `struct { members }`
    Struct(Vec<Member>),
    /// `enum { variants; members }`
    Enum(Vec<Variant>, Vec<Member>),
    /// `(params) -> result { body }`
    Function(Function),
    /// `[captures] (params) -> result { body }`
    Closure(Vec<Expr>, Function),
    /// `object.property`
    Member(Box<Expr>, Property),
    /// `.name`: looked up in the expected type.
    DotName(String),
    /// `callee(args)`
    Call(Box<Expr>, Vec<Expr>),
    /// `object[subscript]`
    Index(Box<Expr>, Box<Expr>),
    /// A prefix operator, as `?T`, `*T`, `-x` and `unsafe F`: the token of a table of the grammar.
    Prefix(TokenKind, Box<Expr>),
    /// A postfix operator, `x.*` and `x?`.
    Postfix(TokenKind, Box<Expr>),
    /// `left op right`, `left..` when open ended.
    Infix(TokenKind, Box<Expr>, Option<Box<Expr>>),
    /// `&qualifier mode operand`
    Borrow(Borrow, Box<Expr>),
    /// `place op value`
    Assign(TokenKind, Box<Expr>, Box<Expr>),
    /// A binding, as a statement, or a pattern binding what it matches whole.
    Bind(Box<Bind>),
    /// `if condition { } else otherwise`, the otherwise a block or an `if`.
    If(Box<Expr>, Block, Option<Box<Expr>>),
    /// `pattern = value`, a head matching.
    Test(Box<Expr>, Box<Expr>),
    /// `for head { }`; without a head, `for { }`.
    For(Option<Box<Expr>>, Block),
    /// `pattern in value`, a head iterating.
    In(Box<Expr>, Box<Expr>),
    /// `match scrutinee { arms }`
    Match(Box<Expr>, Vec<Arm>),
    /// `'label: body`
    Labeled(String, Box<Expr>),
    Return(Option<Box<Expr>>),
    /// `break 'label value`
    Break(Option<String>, Option<Box<Expr>>),
    Continue(Option<String>),
    Yield(Box<Expr>),
    Defer(Box<Expr>),
    /// `#[attrs] inner`
    Attributed(Vec<Expr>, Box<Expr>),
    /// `$for` or `$if` among the entries of a list.
    Staged(Box<Expr>),
}

impl Expr {
    /// The declaration `decl`, by its path: `lang.int.unsigned`.
    #[must_use]
    pub fn decl(decl: Decl) -> Self {
        Self::Path(decl.path().iter().map(|&n| n.to_owned()).collect())
    }

    /// A name declared in the file made: a local path of one segment.
    #[must_use]
    pub fn local(name: &str) -> Self {
        Self::Path(vec![name.to_owned()])
    }

    /// The integer `n`, as a literal.
    #[must_use]
    pub fn int(n: impl Into<Integer>) -> Self {
        Self::Int(n.into())
    }

    /// The float `value`, as a literal.
    #[must_use]
    pub fn float(value: f64) -> Self {
        Self::Float(value.to_bits())
    }

    /// `callee(args)`
    #[must_use]
    pub fn call(callee: Self, args: Vec<Self>) -> Self {
        Self::Call(Box::new(callee), args)
    }

    /// `token inner`
    #[must_use]
    pub fn prefix(token: TokenKind, inner: Self) -> Self {
        Self::Prefix(token, Box::new(inner))
    }

    /// `inner token`
    #[must_use]
    pub fn postfix(token: TokenKind, inner: Self) -> Self {
        Self::Postfix(token, Box::new(inner))
    }

    /// `left token right`
    #[must_use]
    pub fn infix(token: TokenKind, left: Self, right: Self) -> Self {
        Self::Infix(token, Box::new(left), Some(Box::new(right)))
    }

    /// `place token value`
    #[must_use]
    pub fn assign(token: TokenKind, place: Self, value: Self) -> Self {
        Self::Assign(token, Box::new(place), Box::new(value))
    }

    /// `object.name`
    #[must_use]
    pub fn member(object: Self, name: &str) -> Self {
        Self::Member(Box::new(object), Property::Name(name.to_owned()))
    }

    /// `object.index`
    #[must_use]
    pub fn field_at(object: Self, index: u32) -> Self {
        Self::Member(Box::new(object), Property::Index(index))
    }

    /// `object.name(args)`
    #[must_use]
    pub fn method(object: Self, name: &str, args: Vec<Self>) -> Self {
        Self::call(Self::member(object, name), args)
    }

    /// `object[subscript]`
    #[must_use]
    pub fn index(object: Self, subscript: Self) -> Self {
        Self::Index(Box::new(object), Box::new(subscript))
    }

    /// `&mode inner`
    #[must_use]
    pub fn borrow(borrow: Borrow, inner: Self) -> Self {
        Self::Borrow(borrow, Box::new(inner))
    }

    /// `{ stmts; tail }`
    #[must_use]
    pub fn block(stmts: Vec<Self>, tail: Option<Self>) -> Self {
        Self::Block(Block::new(stmts, tail))
    }

    /// `pattern := value`, `name: ty = value` and the like, as a statement.
    #[must_use]
    pub fn bind(bind: Bind) -> Self {
        Self::Bind(Box::new(bind))
    }

    /// `if condition then else otherwise`
    #[must_use]
    pub fn when(condition: Self, then: Block, otherwise: Option<Self>) -> Self {
        Self::If(Box::new(condition), then, otherwise.map(Box::new))
    }

    /// `match scrutinee { arms }`
    #[must_use]
    pub fn matching(scrutinee: Self, arms: Vec<Arm>) -> Self {
        Self::Match(Box::new(scrutinee), arms)
    }

    /// `.name(payload)`
    #[must_use]
    pub fn variant(name: &str, payload: Vec<Self>) -> Self {
        let name = Self::DotName(name.to_owned());
        if payload.is_empty() { name } else { Self::call(name, payload) }
    }

    /// `attrs inner`
    #[must_use]
    pub fn attributed(attrs: Vec<Self>, inner: Self) -> Self {
        Self::Attributed(attrs, Box::new(inner))
    }
}
