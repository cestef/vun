//! Coroutine frames at compile time (spec 06): a call of a coroutine function gives its frame
//! without running it; resuming the frame runs its body from where it stopped until it yields
//! or returns, and stores where it stopped back in the frame.

use std::sync::Arc;

use vun_index::{Idx, IndexVec};
use vun_mir::{BlockId, Body, Mir, Place};
use vun_query::Cx;
use vun_sema::eval::Instantiate;
use vun_sema::fuel::Fueled;
use vun_sema_ty::call::{Instance, Instantiation, Resumed, Suspended};
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Ty, Value};

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
        let env = Instantiation::running(cx, &suspended.call).ok_or(Unevaluated::Absent)?;
        let locals = IndexVec::from(suspended.locals.clone());
        Ok(Self { body, env, locals, steps: 0, fuel: suspended.call.f.fuel(cx) })
    }

    /// Resumes the frame at `place` until it yields or returns, as a value of `step`; its steps
    /// count as this call's.
    pub(crate) fn resume(&mut self, place: &Place, step: Ty) -> Result<Value, Unevaluated> {
        let (suspended, body) = self.suspended(place)?;
        // A frame that returned is not resumed again.
        let at = suspended.at.ok_or(Unevaluated::Unsupported)?;
        let (exit, locals) = self.run_inner(&body, &suspended, Idx::new(at))?;
        let (resumed, at) = match exit {
            Exit::Yielded { value, resume } => (Resumed::Yielded(value), Some(resume.index())),
            Exit::Returned(value) => (Resumed::Returned(value), None),
            Exit::Ended => return Err(Unevaluated::Unsupported),
        };
        self.write(place, Value::Frame(Box::new(Suspended { call: suspended.call, at, locals })))?;
        resumed.value(step).ok_or(Unevaluated::Unsupported)
    }

    /// Ends the frame at `place`: one suspended at a `yield` runs what is pending there; one
    /// that returned, or never started, has nothing pending (spec 06, section 1).
    pub(crate) fn end(&mut self, place: &Place) -> Result<Value, Unevaluated> {
        let (suspended, body) = self.suspended(place)?;
        let cancel = suspended.at.and_then(|at| body.cancel_of(Idx::new(at)));
        if let Some(cancel) = cancel {
            let (exit, locals) = self.run_inner(&body, &suspended, cancel)?;
            if !matches!(exit, Exit::Ended) {
                return Err(Unevaluated::Unsupported);
            }
            self.write(place, Value::Frame(Box::new(Suspended { call: suspended.call, at: None, locals })))?;
        }
        Ok(Value::Tuple(Vec::new()))
    }

    /// The frame at `place`, and the body it runs.
    fn suspended(&self, place: &Place) -> Result<(Suspended, Arc<Body>), Unevaluated> {
        let Value::Frame(suspended) = self.read(place)? else { return Err(Unevaluated::Unsupported) };
        let cx = self.env.cx;
        let instantiation = Instantiation::of(cx, &suspended.call).ok_or(Unevaluated::Absent)?;
        let body = cx.get::<Mir>(&instantiation).map_err(|_| Unevaluated::Unsupported)?;
        Ok((*suspended, body))
    }

    /// Runs `suspended`, a frame of `body`, from `start`, counting its steps as this call's; how
    /// it stopped, and its locals then.
    fn run_inner(
        &mut self,
        body: &Body,
        suspended: &Suspended,
        start: BlockId,
    ) -> Result<(Exit, Vec<Option<Value>>), Unevaluated> {
        let mut inner = Frame::resuming(self.env.cx, body, suspended)?;
        let exit = inner.execute(start);
        self.spend(inner.steps)?;
        Ok((exit?, inner.locals.into_vec()))
    }
}
