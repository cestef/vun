//! The reference interpreter: compile-time evaluation, tests, semantics.
//!
//! It interprets MIR under an explicit target model, and is the definition every backend is
//! differential-tested against (spec 13, section 6).

pub mod bytecode;
pub mod machine;
pub mod memory;

use overt_sema::eval::{Evaluator, FnRef, Trap, Value};
use overt_target::TargetSpec;

pub struct Interpreter {
    target: TargetSpec,
}

impl Interpreter {
    #[must_use]
    pub fn new(target: TargetSpec) -> Self {
        Self { target }
    }

    #[must_use]
    pub fn target(&self) -> &TargetSpec {
        &self.target
    }
}

impl Evaluator for Interpreter {
    fn call(&mut self, _f: FnRef, _args: &[Value]) -> Result<Value, Trap> {
        Err(Trap::Unsupported("evaluation of MIR bodies"))
    }
}
