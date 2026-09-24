//! Running a MIR body on compile-time values: locals hold values, statements compute them, and
//! calls go through sema's memoized `Call` query, so recursion and repeated calls are shared.

use overt_index::IndexVec;
use overt_mir::{Aggregate, Body, Local, Operand, Rvalue, TerminatorKind};
use overt_query::Cx;
use overt_sema::env::{Env, Unevaluated};
use overt_sema::eval::{Instance, Metered, MeteredCall};
use overt_sema::trap::Reason;
use overt_sema::ty::Value;

/// One call's locals.
pub struct Frame<'b, 'a, 'db> {
    body: &'b Body,
    /// The environment of the call, which identifies the types its literals evaluate to.
    env: Env<'a, 'db>,
    locals: IndexVec<Local, Option<Value>>,
    /// The steps taken so far: statements and terminators of this body, and the steps of the
    /// calls it made.
    steps: u64,
    /// The steps the call may take.
    fuel: u64,
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
        let value = self.execute();
        Metered { value, steps: self.steps }
    }

    fn execute(&mut self) -> Result<Value, Unevaluated> {
        let mut block = &self.body.blocks[overt_index::Idx::new(0)];
        loop {
            self.spend(1)?;
            for stmt in &block.stmts {
                self.spend(1)?;
                let value = self.rvalue(&stmt.value).map_err(|e| e.through(stmt.origin))?;
                self.locals[stmt.place.local] = Some(value);
            }
            match &block.term.kind {
                TerminatorKind::Return => {
                    return self.locals[self.body.ret].take().ok_or(Unevaluated::Unsupported);
                }
                TerminatorKind::Goto(to) => block = &self.body.blocks[*to],
                TerminatorKind::Branch { cond, then, otherwise } => {
                    let next = if self.env.holds(&self.operand(cond)?)? { then } else { otherwise };
                    block = &self.body.blocks[*next];
                }
            }
        }
    }

    /// Counts `steps` more, trapping past the call's budget.
    fn spend(&mut self, steps: u64) -> Result<(), Unevaluated> {
        self.steps = self.steps.saturating_add(steps);
        if self.steps > self.fuel {
            return Err(Unevaluated::trap(Reason::Budget).through(self.body.origin));
        }
        Ok(())
    }

    fn operand(&self, operand: &Operand) -> Result<Value, Unevaluated> {
        match operand {
            Operand::Place(place) => self.locals[place.local].clone().ok_or(Unevaluated::Unsupported),
            Operand::Const(value) => Ok(value.clone()),
        }
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
            Rvalue::Comptime(expr) => self.env.eval(*expr),
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
                })
            }
            Rvalue::Field { operand, name } => self.env.member(self.operand(operand)?, *name),
            Rvalue::Index { operand, index } => self.operand(operand)?.item(&self.operand(index)?),
            Rvalue::Call { callee, args } => {
                let args = args.iter().map(|a| self.operand(a)).collect::<Result<Vec<_>, _>>()?;
                match self.operand(callee)? {
                    // A call of a function counts the steps it took (spec 03, section 7).
                    Value::Fn(f) | Value::Interface(f) => {
                        let called = self.env.cx.get::<MeteredCall>(&Instance { f, args });
                        self.spend(called.steps)?;
                        called.value
                    }
                    callee => self.env.apply(callee, args),
                }
            }
        }
    }
}
