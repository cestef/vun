//! The reference interpreter: compile-time evaluation, tests, semantics (spec 13, section 6).
//!
//! It interprets MIR and is the definition every backend is differential-tested against. So far
//! it runs compile-time bodies on compile-time values; the target byte model, the register
//! bytecode comes with run-time values.

pub mod bytecode;
mod frames;
mod loans;
pub mod machine;
pub mod memory;
mod places;

use machine::Frame;
use vun_mir::Mir;
use vun_query::Cx;
use vun_sema::eval::Instantiate;
use vun_sema::eval::{Evaluator, Metered};
use vun_sema_ty::call::Instance;
use vun_sema_ty::trap::Unevaluated;

/// The evaluator sema calls through, installed by the driver.
pub struct Interpreter;

impl Evaluator for Interpreter {
    fn call(&self, cx: &Cx<'_>, call: &Instance) -> Metered {
        let Some(instantiation) = call.instantiation().body(cx) else {
            return Metered::unrun(Unevaluated::Absent);
        };
        let body = match cx.get::<Mir>(&instantiation) {
            Ok(body) => body,
            Err(unbuilt) => return Metered::unrun(Unevaluated::Unbuilt(unbuilt.at)),
        };
        match Frame::new(cx, &body, call) {
            // A call of a coroutine function gives its frame (spec 06, section 1).
            Ok(frame) if body.suspends() => {
                Metered { value: Ok(frame.suspend(call)), steps: 1, written: Vec::new() }
            }
            Ok(frame) => frame.run(),
            Err(e) => Metered::unrun(e),
        }
    }
}
