//! The forms of statements.

use vun_grammar::TokenKind;

use super::{Expr, FnId, LocalId, Place};

/// How a binding is written: with the type of what it binds, which the compiler infers, or with
/// a type of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Binding {
    Inferred,
    Annotated,
}

/// The assignment forms of an integer place that cannot trap: plain, wrapping and saturating.
#[derive(Clone, Copy, Debug, PartialEq, Eq, vun_variants::Variants)]
pub enum Assign {
    Set,
    AddWrap,
    SubWrap,
    MulWrap,
    AddSat,
    SubSat,
    MulSat,
}

impl Assign {
    /// The operator's token.
    #[must_use]
    pub const fn token(self) -> TokenKind {
        match self {
            Self::Set => TokenKind::Eq,
            Self::AddWrap => TokenKind::PlusPercentEq,
            Self::SubWrap => TokenKind::MinusPercentEq,
            Self::MulWrap => TokenKind::StarPercentEq,
            Self::AddSat => TokenKind::PlusPipeEq,
            Self::SubSat => TokenKind::MinusPipeEq,
            Self::MulSat => TokenKind::StarPipeEq,
        }
    }
}

/// The name of a loop, which a `break` or `continue` inside it may name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Label(pub usize);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stmt {
    Let {
        local: LocalId,
        init: Expr,
        binding: Binding,
    },
    Assign {
        place: Place,
        op: Assign,
        value: Expr,
    },
    If {
        condition: Expr,
        then: Vec<Stmt>,
        otherwise: Vec<Stmt>,
    },
    /// `for local in 0..count { body }`, `local` the counter.
    Count {
        local: LocalId,
        count: u32,
        body: Vec<Stmt>,
    },
    /// `for local in array { body }`: over the items of an array held in `array`.
    Each {
        local: LocalId,
        array: LocalId,
        body: Vec<Stmt>,
    },
    /// `for condition { body }`
    While {
        condition: Expr,
        body: Vec<Stmt>,
    },
    /// `for .some(local) = option { body }`: while the option, given anew each time, holds a value.
    WhileSome {
        local: LocalId,
        option: Expr,
        body: Vec<Stmt>,
    },
    /// `for local in function(args) { body }`: over the values an iterator function yields.
    Drain {
        local: LocalId,
        function: FnId,
        args: Vec<Expr>,
        body: Vec<Stmt>,
    },
    /// `yield value`, in an iterator function.
    Yield(Expr),
    /// `defer stmt`: run at every exit of the block, the latest first.
    Defer(Box<Stmt>),
    /// `'label: loop`, the loop being the statement.
    Labeled(Label, Box<Stmt>),
    /// `break 'label value`, to the loop the label names or the innermost one.
    Break(Option<Label>, Option<Expr>),
    Continue(Option<Label>),
}

impl Stmt {
    /// The statement lists directly inside it.
    #[must_use]
    pub fn bodies(&self) -> Vec<&[Stmt]> {
        match self {
            Self::If { then, otherwise, .. } => vec![then, otherwise],
            Self::Count { body, .. }
            | Self::Each { body, .. }
            | Self::While { body, .. }
            | Self::WhileSome { body, .. }
            | Self::Drain { body, .. } => vec![body],
            Self::Labeled(_, inner) | Self::Defer(inner) => inner.bodies(),
            Self::Let { .. } | Self::Assign { .. } | Self::Yield(_) | Self::Break(..) | Self::Continue(_) => {
                Vec::new()
            }
        }
    }
}
