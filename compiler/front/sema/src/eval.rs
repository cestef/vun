//! The interface to compile-time evaluation.
//!
//! Sema defines what it needs from an evaluator; `overt-interp` implements it. A JIT can later be
//! added behind the same trait (spec 13, section 6).

/// A function known to sema, by index into its function table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FnRef(pub u32);

/// A compile-time value, by index into the intern pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Value(pub u32);

/// Why evaluation stopped without a result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Trap {
    /// The step budget of the evaluation root was exhausted.
    OutOfFuel,
    /// A trap site fired; the id indexes the trap-site table.
    Site(u32),
    /// The evaluator does not support this construct yet.
    Unsupported(&'static str),
}

pub trait Evaluator {
    /// Calls `f` with `args`.
    ///
    /// # Errors
    /// Returns the trap that stopped evaluation.
    fn call(&mut self, f: FnRef, args: &[Value]) -> Result<Value, Trap>;
}
