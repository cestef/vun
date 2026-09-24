//! The reference interpreter: compile-time evaluation, tests, semantics (spec 13, section 6).
//!
//! It interprets MIR and is the definition every backend is differential-tested against. So far
//! it runs compile-time bodies on compile-time values; the target byte model, the register
//! bytecode comes with run-time values.

pub mod bytecode;
mod frames;
pub mod machine;
pub mod memory;

use overt_mir::Mir;
use overt_query::Cx;
use overt_sema::env::Unevaluated;
use overt_sema::eval::{Evaluator, Instance, Instantiation, Metered};

use machine::Frame;

/// The evaluator sema calls through, installed by the driver.
pub struct Interpreter;

impl Evaluator for Interpreter {
    fn call(&self, cx: &Cx<'_>, call: &Instance) -> Metered {
        let unrun = |value| Metered { value: Err(value), steps: 0 };
        let Some(instantiation) = Instantiation::of(cx, call) else { return unrun(Unevaluated::Absent) };
        let Ok(body) = cx.get::<Mir>(&instantiation) else { return unrun(Unevaluated::Unsupported) };
        match Frame::new(cx, &body, call) {
            // A call of a coroutine function gives its frame (spec 06, section 1).
            Ok(frame) if body.suspends() => Metered { value: Ok(frame.suspend(call)), steps: 1 },
            Ok(frame) => frame.run(),
            Err(e) => unrun(e),
        }
    }
}
