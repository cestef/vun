//! Running a MIR body on compile-time values: locals hold values, statements compute them, and
//! calls go through sema's memoized `Call` query, so recursion and repeated calls are shared.

use overt_index::{Idx, IndexVec};
use overt_mir::{Aggregate, BlockId, Body, Local, Operand, Place, Projection, Rvalue, TerminatorKind};
use overt_query::Cx;
use overt_sema::env::{Env, Unevaluated};
use overt_sema::eval::{Instance, Metered, MeteredCall};
use overt_sema::ops::Part;
use overt_sema::trap::Reason;
use overt_sema::ty::Value;

/// One call's locals.
pub struct Frame<'b, 'a, 'db> {
    pub(crate) body: &'b Body,
    /// The environment of the call, which identifies the types its literals evaluate to.
    pub(crate) env: Env<'a, 'db>,
    pub(crate) locals: IndexVec<Local, Option<Value>>,
    /// The steps taken so far: statements and terminators of this body, and the steps of the
    /// calls it made.
    pub(crate) steps: u64,
    /// The steps the call may take.
    pub(crate) fuel: u64,
}

/// How a run of a body stops.
pub(crate) enum Exit {
    Returned(Value),
    /// At a `yield`, to go on at `resume`.
    Yielded {
        value: Value,
        resume: BlockId,
    },
}

impl<'b, 'a, 'db> Frame<'b, 'a, 'db> {
    /// A frame running `body` for `call`, its arguments in the parameters.
    ///
    /// # Errors
    /// Returns [`Unevaluated::Unsupported`] when the arguments do not match the parameters.
    pub fn new(cx: &'a Cx<'db>, body: &'b Body, call: &Instance) -> Result<Self, Unevaluated> {
        if call.args.len() != body.params.len() {
            return Err(Unevaluated::Unsupported);
        }
        let env = Env::within(cx, call).ok_or(Unevaluated::Absent)?;
        let mut locals = IndexVec::filled(None, body.locals.len());
        for (&param, arg) in body.params.iter().zip(&call.args) {
            locals[param] = Some(arg.clone());
        }
        Ok(Self { body, env, locals, steps: 0, fuel: call.f.fuel(cx) })
    }

    /// Runs the body to its return, and gives the result with the steps taken.
    #[must_use]
    pub fn run(mut self) -> Metered {
        let value = match self.execute(Idx::new(0)) {
            Ok(Exit::Returned(value)) => Ok(value),
            // A body that suspends is a coroutine's, whose calls give frames instead.
            Ok(Exit::Yielded { .. }) => Err(Unevaluated::Unsupported),
            Err(e) => Err(e),
        };
        Metered { value, steps: self.steps }
    }

    /// Runs the body from `start` until it returns or yields.
    pub(crate) fn execute(&mut self, start: BlockId) -> Result<Exit, Unevaluated> {
        let mut block = &self.body.blocks[start];
        loop {
            self.spend(1)?;
            for stmt in &block.stmts {
                self.spend(1)?;
                let value = self.rvalue(&stmt.value).map_err(|e| e.through(stmt.origin))?;
                self.write(&stmt.place, value).map_err(|e| e.through(stmt.origin))?;
            }
            match &block.term.kind {
                TerminatorKind::Return => {
                    let value = self.locals[self.body.ret].take().ok_or(Unevaluated::Unsupported)?;
                    return Ok(Exit::Returned(value));
                }
                TerminatorKind::Yield { value, resume } => {
                    return Ok(Exit::Yielded { value: self.operand(value)?, resume: *resume });
                }
                TerminatorKind::Goto(to) => block = &self.body.blocks[*to],
                TerminatorKind::Branch { cond, then, otherwise } => {
                    let next = if self.env.holds(&self.operand(cond)?)? { then } else { otherwise };
                    block = &self.body.blocks[*next];
                }
                // Linear values are not tracked at compile time, so only a deferred statement
                // consuming none is known to run.
                TerminatorKind::Defer { consumes, run, .. } if consumes.is_empty() => {
                    block = &self.body.blocks[*run];
                }
                TerminatorKind::Unreachable | TerminatorKind::Defer { .. } => {
                    return Err(Unevaluated::Unsupported);
                }
            }
        }
    }

    /// Counts `steps` more, trapping past the call's budget.
    pub(crate) fn spend(&mut self, steps: u64) -> Result<(), Unevaluated> {
        self.steps = self.steps.saturating_add(steps);
        if self.steps > self.fuel {
            return Err(Unevaluated::trap(Reason::Budget).through(self.body.origin));
        }
        Ok(())
    }

    fn operand(&self, operand: &Operand) -> Result<Value, Unevaluated> {
        match operand {
            Operand::Place(place) => self.read(place),
            Operand::Const(value) => Ok(value.clone()),
        }
    }

    fn local(&self, local: Local) -> Result<Value, Unevaluated> {
        self.locals[local].clone().ok_or(Unevaluated::Unsupported)
    }

    /// The value at `place`. A borrow at compile time is shared, and a shared borrow of a
    /// compile-time value is the value, so `.*` reads it.
    pub(crate) fn read(&self, place: &Place) -> Result<Value, Unevaluated> {
        let mut value = self.local(place.local)?;
        for projection in &place.projection {
            value = match self.part(projection)? {
                Some(part) => value.part(&part)?,
                None => value,
            };
        }
        Ok(value)
    }

    /// Stores `value` at `place`: a whole local, or a part of one, rebuilt around it.
    pub(crate) fn write(&mut self, place: &Place, value: Value) -> Result<(), Unevaluated> {
        let value = if place.projection.is_empty() {
            value
        } else {
            self.replaced(self.local(place.local)?, &place.projection, value)?
        };
        self.locals[place.local] = Some(value);
        Ok(())
    }

    /// `whole` with the part `projection` reaches replaced by `part`. Writing through `.*` needs
    /// mutable borrows, which compile-time evaluation does not have yet.
    fn replaced(&self, whole: Value, projection: &[Projection], value: Value) -> Result<Value, Unevaluated> {
        let Some((first, rest)) = projection.split_first() else { return Ok(value) };
        let part = self.part(first)?.ok_or(Unevaluated::Unsupported)?;
        let inner = self.replaced(whole.part(&part)?, rest, value)?;
        whole.with_part(&part, inner)
    }

    /// The part of a value `projection` reaches, its index read; `None` for `.*`.
    fn part(&self, projection: &Projection) -> Result<Option<Part>, Unevaluated> {
        Ok(Some(match projection {
            Projection::Deref => return Ok(None),
            Projection::Field(name) => Part::Field(*name),
            Projection::Index(index) => Part::Item(self.local(*index)?),
            Projection::Position(at) => Part::Position(*at),
            Projection::Payload(at) => Part::Payload(*at),
        }))
    }

    fn rvalue(&mut self, rvalue: &Rvalue) -> Result<Value, Unevaluated> {
        match rvalue {
            Rvalue::Use(operand) => self.operand(operand),
            Rvalue::Binary { op, left, right } => {
                self.env.binary(*op, &self.operand(left)?, &self.operand(right)?)
            }
            Rvalue::Unary { op, operand } => self.env.unary(*op, &self.operand(operand)?),
            Rvalue::Range { op, lo, hi } => {
                let hi = hi.as_ref().map(|h| self.operand(h)).transpose()?;
                Value::range(*op, &self.operand(lo)?, hi.as_ref())
            }
            Rvalue::Narrow { operand, ty, narrowing } => {
                self.operand(operand)?.narrow(&ty.interval().ok_or(Unevaluated::Unsupported)?, *narrowing)
            }
            Rvalue::Comptime { expr, staged } => self.env.clone().staging(staged).eval(*expr),
            Rvalue::Quote { expr, staged } => Ok(Value::Code(self.env.clone().staging(staged).code(*expr))),
            Rvalue::Aggregate { kind, parts } => {
                let parts = parts.iter().map(|p| self.operand(p)).collect::<Result<Vec<_>, _>>()?;
                Ok(match kind {
                    Aggregate::Struct { ty, fields } => {
                        Value::Struct { ty: *ty, fields: fields.iter().copied().zip(parts).collect() }
                    }
                    Aggregate::Variant { ty, name } => {
                        Value::Variant { ty: *ty, name: *name, payload: parts }
                    }
                    Aggregate::Tuple => Value::Tuple(parts),
                    Aggregate::Array => Value::Array(parts),
                    Aggregate::Closure { literal, staged } => {
                        Value::Closure { f: self.env.clone().staging(staged).func(*literal), captures: parts }
                    }
                })
            }
            Rvalue::Ref { mode: None, place } => self.read(place),
            Rvalue::Resume { frame, step } => self.resume(frame, *step),
            Rvalue::Repeat { operand, count } => Value::repeat(self.operand(operand)?, count),
            Rvalue::IsVariant { place, variant } => match self.read(place)? {
                Value::Variant { name, .. } => self.env.decide(name == *variant),
                _ => Err(Unevaluated::Unsupported),
            },
            // A mutable borrow at compile time needs a memory model of places.
            Rvalue::Ref { mode: Some(_), .. } => Err(Unevaluated::Unsupported),
            Rvalue::Call { callee, args } => {
                let args = args.iter().map(|a| self.operand(a)).collect::<Result<Vec<_>, _>>()?;
                // A call of a function counts the steps it took (spec 03, section 7); a closure
                // passes its captures first.
                let (f, args) = match self.operand(callee)? {
                    Value::Fn(f) | Value::Interface(f) => (f, args),
                    Value::Closure { f, captures } => (f, captures.into_iter().chain(args).collect()),
                    callee => return self.env.apply(callee, args),
                };
                let called = self.env.cx.get::<MeteredCall>(&Instance { f, args });
                self.spend(called.steps)?;
                called.value
            }
        }
    }
}
