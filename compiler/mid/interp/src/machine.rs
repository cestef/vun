//! Running a MIR body on compile-time values: locals hold values, statements compute them, and
//! calls go through sema's memoized `Call` query, so recursion and repeated calls are shared.

use overt_index::IndexVec;
use overt_mir::{Body, Local, Operand, Rvalue, TerminatorKind};
use overt_query::Cx;
use overt_sema::env::{Env, Unevaluated};
use overt_sema::eval::{Call, Instance};
use overt_sema::ty::Value;

/// One call's locals.
pub struct Frame<'b, 'a, 'db> {
    body: &'b Body,
    /// The environment of the call, which identifies the types its literals evaluate to.
    env: Env<'a, 'db>,
    locals: IndexVec<Local, Option<Value>>,
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
        Ok(Self { body, env, locals })
    }

    /// Runs the body to its return, and gives the result.
    ///
    /// # Errors
    /// Returns why a statement has no value.
    pub fn run(mut self) -> Result<Value, Unevaluated> {
        let mut block = &self.body.blocks[overt_index::Idx::new(0)];
        loop {
            for stmt in &block.stmts {
                let value = self.rvalue(&stmt.value)?;
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

    fn operand(&self, operand: &Operand) -> Result<Value, Unevaluated> {
        match operand {
            Operand::Place(place) => self.locals[place.local].clone().ok_or(Unevaluated::Unsupported),
            Operand::Const(value) => Ok(value.clone()),
        }
    }

    fn rvalue(&self, rvalue: &Rvalue) -> Result<Value, Unevaluated> {
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
            Rvalue::Nominal { literal, kind } => Ok(self.env.nominal(*literal, *kind)),
            Rvalue::Call { callee, args } => {
                let args = args.iter().map(|a| self.operand(a)).collect::<Result<Vec<_>, _>>()?;
                match self.operand(callee)? {
                    Value::Fn(f) => self.env.cx.get::<Call>(&Instance { f, args }),
                    Value::Primitive(p) => p.apply(&args),
                    _ => Err(Unevaluated::Unsupported),
                }
            }
        }
    }
}
