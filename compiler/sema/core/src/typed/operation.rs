//! Where typing found operators and iterations, and what they call.

use vun_codec::{Decode, Encode};
use vun_hir::tree::{ExprId, PatId};
use vun_intern::Symbol;
use vun_sema_ty::{Ty, Value};

/// Where an operator is written, by what it belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
pub enum Operation {
    /// `a op b` or `op a`.
    Expr(ExprId),
    /// `x op= y`, by its place `x`.
    Assign(ExprId),
    /// A value pattern: the value matched `==` the pattern's.
    Test(PatId),
    /// A range pattern's lower end: `lo <=` the value matched.
    Lower(PatId),
    /// A range pattern's upper end: the value matched `<` or `<=` `hi`.
    Upper(PatId),
}

/// The function an operator calls: the struct declaring it and its name, and the `$` arguments
/// inferred for the call, by parameter.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Operator {
    pub method: (Ty, Symbol),
    pub inferred: Vec<(usize, Value)>,
    /// The type of the call's result.
    pub result: Ty,
}

/// How `for p in e` drives `e` (the table `iteration`): the witness of `lang.ops.Iterate` whose
/// `iter` makes the iterator, and the type its `resume` gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Iteration {
    pub witness: Ty,
    pub step: Ty,
}
