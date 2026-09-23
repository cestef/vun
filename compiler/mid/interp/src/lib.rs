//! The reference interpreter: compile-time evaluation, tests, semantics (spec 13, section 6).
//!
//! It interprets MIR and is the definition every backend is differential-tested against. So far
//! it runs compile-time bodies on compile-time values; the target byte model, the register
//! bytecode and the step budget come with run-time values.

pub mod bytecode;
pub mod machine;
pub mod memory;

use overt_mir::Mir;
use overt_query::Cx;
use overt_sema::env::Unevaluated;
use overt_sema::eval::{Evaluator, FnRef};
use overt_sema::ty::Value;

use machine::Frame;

/// The evaluator sema calls through, installed by the driver.
pub struct Interpreter;

impl Evaluator for Interpreter {
    fn call(&self, cx: &Cx<'_>, f: &FnRef, args: &[Value]) -> Result<Value, Unevaluated> {
        let body = cx.get::<Mir>(f).map_err(|_| Unevaluated::Unsupported)?;
        Frame::new(&body, args)?.run(cx)
    }
}
