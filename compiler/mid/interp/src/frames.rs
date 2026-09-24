//! Coroutine frames at compile time (spec 06): a call of a coroutine function gives its frame
//! without running it; resuming the frame runs its body from where it stopped until it yields
//! or returns, and stores where it stopped back in the frame.

use overt_index::{Idx, IndexVec};
use overt_mir::{Body, Mir, Place};
use overt_query::Cx;
use overt_sema::env::{Env, Unevaluated};
use overt_sema::eval::{Instance, Instantiation, Resumed, Suspended};
use overt_sema::ty::{Ty, Value};

use crate::machine::{Exit, Frame};

impl<'b, 'a, 'db> Frame<'b, 'a, 'db> {
    /// This frame, not started, as a frame value of the call `call`.
    #[must_use]
    pub fn suspend(self, call: &Instance) -> Value {
        let locals = self.locals.into_vec();
        Value::Frame(Box::new(Suspended { call: call.clone(), at: Some(0), locals }))
    }

    /// A frame going on with `suspended`, a call of `body`.
    fn resuming(cx: &'a Cx<'db>, body: &'b Body, suspended: &Suspended) -> Result<Self, Unevaluated> {
        let env = Env::within(cx, &suspended.call).ok_or(Unevaluated::Absent)?;
        let locals = IndexVec::from(suspended.locals.clone());
        Ok(Self { body, env, locals, steps: 0, fuel: suspended.call.f.fuel(cx) })
    }

    /// Resumes the frame at `place` until it yields or returns, as a value of `step`; its steps
    /// count as this call's.
    pub(crate) fn resume(&mut self, place: &Place, step: Ty) -> Result<Value, Unevaluated> {
        let Value::Frame(suspended) = self.read(place)? else { return Err(Unevaluated::Unsupported) };
        // A frame that returned is not resumed again.
        let at = suspended.at.ok_or(Unevaluated::Unsupported)?;
        let cx = self.env.cx;
        let instantiation = Instantiation::of(cx, &suspended.call).ok_or(Unevaluated::Absent)?;
        let body = cx.get::<Mir>(&instantiation).map_err(|_| Unevaluated::Unsupported)?;
        let mut inner = Frame::resuming(cx, &body, &suspended)?;
        let exit = inner.execute(Idx::new(at));
        self.spend(inner.steps)?;
        let (resumed, at) = match exit? {
            Exit::Yielded { value, resume } => (Resumed::Yielded(value), Some(resume.index())),
            Exit::Returned(value) => (Resumed::Returned(value), None),
        };
        let locals = inner.locals.into_vec();
        self.write(place, Value::Frame(Box::new(Suspended { call: suspended.call, at, locals })))?;
        resumed.value(step).ok_or(Unevaluated::Unsupported)
    }
}
