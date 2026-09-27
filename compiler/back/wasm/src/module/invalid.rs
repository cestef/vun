//! Why a module is invalid, as the verifier finds it.

use std::error::Error;
use std::fmt;

use super::op::Mem;
use super::{SymbolId, TypeId, ValType};

/// Why a module is invalid, and where: the defined function and the instruction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invalid {
    pub func: usize,
    pub at: usize,
    pub why: Why,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Why {
    /// An instruction popped an operand the stack of its block did not hold.
    Underflow,
    /// An operand of another type than the instruction pops.
    Mismatch { expected: ValType, found: ValType },
    /// A branch to a label no block around it has.
    Label(u32),
    /// A local past the function's.
    Local(u32),
    /// A type the module does not list.
    Type(TypeId),
    /// A symbol naming something other than the instruction needs.
    Symbol(SymbolId),
    /// A load or store of a width the backend has no instruction for.
    Access(Mem),
    /// An `else` outside an `if`, or an `end` closing nothing.
    Unbalanced,
    /// A block ended holding other operands than it gives.
    Leftover,
    /// The body ended with blocks open.
    Unclosed,
}

impl fmt::Display for Invalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "function {}, instruction {}: ", self.func, self.at)?;
        match &self.why {
            Why::Underflow => f.write_str("pops an operand the stack does not hold"),
            Why::Mismatch { expected, found } => write!(f, "expects {expected:?}, finds {found:?}"),
            Why::Label(l) => write!(f, "branches to label {l}, which no block has"),
            Why::Local(l) => write!(f, "names local {l}, past the function's"),
            Why::Type(t) => write!(f, "names type {t:?}, which the module does not list"),
            Why::Symbol(s) => write!(f, "names symbol {s:?}, of another kind"),
            Why::Access(m) => write!(f, "accesses memory as {m:?}, which has no instruction"),
            Why::Unbalanced => f.write_str("closes a block that is not open"),
            Why::Leftover => f.write_str("ends a block holding operands it does not give"),
            Why::Unclosed => f.write_str("ends with blocks open"),
        }
    }
}

impl Error for Invalid {}
