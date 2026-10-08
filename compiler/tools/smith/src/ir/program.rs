//! Functions and the program of them.

use vun_index::{IndexVec, index};

use super::{Expr, Stmt};
use crate::ty::Ty;

index! {
    pub struct LocalId;
    pub struct FnId;
    pub struct TypeId;
}

/// A name a function binds: a parameter, a `let`, a loop counter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Local {
    pub ty: Ty,
    pub mutable: bool,
}

/// What a function asks of one of its `$` type parameters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Bound {
    /// Any type, given with the call.
    Any,
    /// A callable taking these and giving this, which the call's argument fixes.
    Call(Vec<Ty>, Ty),
}

/// What a function is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Called for its value.
    Plain,
    /// A coroutine that yields values of its result type, which a `for` consumes.
    Iterator,
}

/// A function: its parameters are its first locals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Function {
    pub locals: IndexVec<LocalId, Local>,
    pub params: usize,
    /// The `$T` parameters, which come before the others.
    pub generics: Vec<Bound>,
    pub role: Role,
    /// What it gives; of an iterator, what it yields.
    pub result: Ty,
    pub body: Vec<Stmt>,
    pub tail: Expr,
    /// The steps a call takes, as the model counts them.
    pub steps: u64,
}

impl Function {
    /// The types of its parameters.
    #[must_use]
    pub fn signature(&self) -> Vec<Ty> {
        self.locals.iter().take(self.params).map(|(_, l)| l.ty.clone()).collect()
    }
}

/// Functions calling only those before them, and the entry points `main` runs.
#[derive(Clone, Debug, Default)]
pub struct Program {
    /// The structs and enums, each as the type of its values: a type's parts are of earlier ones.
    pub types: IndexVec<TypeId, Ty>,
    pub functions: IndexVec<FnId, Function>,
    /// The functions without parameters whose values `main` folds into what it prints.
    pub roots: Vec<FnId>,
}

impl Program {
    /// The steps the program takes, as the model counts them: those of its roots.
    #[must_use]
    pub fn steps(&self) -> u64 {
        self.roots.iter().map(|&r| self.functions[r].steps).fold(0, u64::saturating_add)
    }
}
