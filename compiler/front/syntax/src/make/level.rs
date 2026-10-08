//! How tightly each made form holds its operands, from the grammar's tables, and the shapes the
//! printer must keep apart from the text around them.

use vun_grammar::{Level, NodeKind, TokenKind};

use super::{Expr, Property};

impl Expr {
    /// The kind of node this form is when read back.
    pub(super) fn kind(&self) -> NodeKind {
        match self {
            Self::Int(n) if n.negative => NodeKind::Prefix,
            Self::Int(_) => NodeKind::Int,
            Self::Float(bits) if f64::from_bits(*bits).is_sign_negative() => NodeKind::Prefix,
            Self::Float(_) => NodeKind::Float,
            Self::Char(_) => NodeKind::Char,
            Self::String(_) => NodeKind::String,
            Self::Hole => NodeKind::Hole,
            Self::Path(path) if path.len() > 1 => NodeKind::Member,
            Self::Path(_) => NodeKind::Name,
            Self::Unit | Self::Tuple(_) => NodeKind::Tuple,
            Self::List(_) => NodeKind::Array,
            Self::Repeat(..) => NodeKind::ArrayRepeat,
            Self::Record(_) => NodeKind::StructLit,
            Self::Typed(..) => NodeKind::TypedLit,
            Self::Block(_) => NodeKind::Block,
            Self::ArrayType(..) => NodeKind::ArrayType,
            Self::FnType(..) => NodeKind::FnType,
            Self::Struct(_) => NodeKind::Struct,
            Self::Enum(..) => NodeKind::Enum,
            Self::Function(_) => NodeKind::Fn,
            Self::Closure(..) => NodeKind::Closure,
            Self::Member(..) => NodeKind::Member,
            Self::DotName(_) => NodeKind::DotName,
            Self::Call(..) => NodeKind::Call,
            Self::Index(..) => NodeKind::Index,
            Self::Prefix(..) | Self::Borrow(..) => NodeKind::Prefix,
            Self::Postfix(..) => NodeKind::Postfix,
            Self::Infix(..) => NodeKind::Infix,
            Self::Assign(..) => NodeKind::Assign,
            Self::Bind(bind) if bind.ty.is_some() => NodeKind::TypedBinding,
            Self::Bind(_) => NodeKind::Binding,
            Self::If(..) => NodeKind::If,
            Self::Test(..) => NodeKind::Test,
            Self::For(..) => NodeKind::For,
            Self::In(..) => NodeKind::In,
            Self::Match(..) => NodeKind::Match,
            Self::Labeled(..) => NodeKind::Labeled,
            Self::Return(_) => NodeKind::Return,
            Self::Break(..) => NodeKind::Break,
            Self::Continue(_) => NodeKind::Continue,
            Self::Yield(_) => NodeKind::Yield,
            Self::Defer(_) => NodeKind::Defer,
            Self::Attributed(..) => NodeKind::Attr,
            Self::Staged(_) => NodeKind::Staged,
        }
    }

    /// How tightly this form holds its operands. A form with no operand at its ends is as tight
    /// as any; one ending in a brace is as loose as the control forms, so that nothing continues
    /// it unparenthesized.
    pub(super) fn level(&self) -> Level {
        match self {
            Self::Infix(token, ..) => token.infix().map_or(Level::LOWEST, |infix| infix.level),
            // What an attribute applies to is its own form's business.
            Self::Attributed(..) => Level::HIGHEST,
            Self::Block(_) | Self::Function(_) | Self::Struct(_) | Self::Enum(..) => Self::value_level(),
            _ => self.kind().level().unwrap_or(Level::HIGHEST),
        }
    }

    /// The level of the form that holds an operand on its left as a postfix operator does.
    pub(super) fn postfix_level() -> Level {
        NodeKind::Member.operand_level()
    }

    /// The level of the form that holds its operand on its right as a prefix operator does.
    pub(super) fn prefix_level() -> Level {
        NodeKind::Prefix.operand_level()
    }

    /// The level of the loosest form an assigned or bound value may be: any but a binding or assignment.
    pub(super) fn value_level() -> Level {
        NodeKind::If.operand_level()
    }

    /// Whether the form writes a `{` that would open the body of a head it stands in.
    pub(super) fn braced(&self) -> bool {
        matches!(
            self,
            Self::Typed(..)
                | Self::Block(_)
                | Self::If(..)
                | Self::For(..)
                | Self::Match(..)
                | Self::Function(_)
                | Self::Closure(..)
                | Self::Struct(_)
                | Self::Enum(..)
                | Self::Labeled(..)
        ) || matches!(self, Self::Prefix(token, inner) if Self::words(*token) && inner.braced())
    }

    /// Whether a statement of this form ends at its closing brace, needing no `;` (spec 01,
    /// section 6): the next token starts a new statement.
    pub(super) fn block_like(&self) -> bool {
        match self {
            Self::Block(_)
            | Self::If(..)
            | Self::For(..)
            | Self::Match(..)
            | Self::Labeled(..)
            | Self::Staged(_) => true,
            Self::Prefix(token, inner) => Self::words(*token) && inner.block_like(),
            Self::Attributed(_, inner) => inner.block_like(),
            _ => false,
        }
    }

    /// Whether the prefix `token` is a word that applies to a braced form as it stands: `unsafe`
    /// and `$`.
    pub(super) fn words(token: TokenKind) -> bool {
        matches!(token, TokenKind::Unsafe | TokenKind::Dollar)
    }

    /// Whether member access reads this as a number followed by a fraction: a literal, or a tuple
    /// index, which `.` would lex into a float with the next index.
    pub(super) fn numeric(&self) -> bool {
        matches!(self, Self::Int(_) | Self::Float(_) | Self::Member(_, Property::Index(_)))
    }
}
