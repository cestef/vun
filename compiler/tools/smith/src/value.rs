//! The values of the model: what an expression of each type is, when the program runs.

use std::cell::RefCell;
use std::rc::Rc;

use crate::float::Float;
use crate::int::BigInt;
use crate::ir::{Closure, FnId, LocalId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Int(BigInt),
    Bool(bool),
    Float(Float),
    Unit,
    /// A closure made: what it is, the function that wrote it, and the variables it captured.
    Closure(Rc<Closure>, FnId, Vec<(LocalId, Cell)>),
    /// A borrow of a local, which lives as long as the call it is passed to.
    Ref(Cell),
    Tuple(Vec<Value>),
    Array(Vec<Value>),
    /// A value of an enum: the variant, and what it holds.
    Variant(usize, Vec<Value>),
}

/// A variable of a running function, which a reference lends.
#[derive(Clone, Debug)]
pub struct Cell(pub Rc<RefCell<Value>>);

impl PartialEq for Cell {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Cell {}

impl Value {
    /// The integer, which a form of the model asked of an integer.
    #[must_use]
    pub fn int(&self) -> Option<&BigInt> {
        match self {
            Self::Int(n) => Some(n),
            _ => None,
        }
    }

    /// The parts, of a tuple or an array.
    #[must_use]
    pub fn parts(&self) -> Option<&[Value]> {
        match self {
            Self::Tuple(parts) | Self::Array(parts) => Some(parts),
            _ => None,
        }
    }
}
