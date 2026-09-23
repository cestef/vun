//! Running a MIR body on compile-time values: locals hold values, statements compute them, and
//! calls go through sema's memoized `Call` query, so recursion and repeated calls are shared.

use overt_index::IndexVec;
use overt_mir::{Body, Local, Operand, Rvalue, TerminatorKind};
use overt_query::Cx;
use overt_sema::env::Unevaluated;
use overt_sema::eval::Call;
use overt_sema::ty::Value;

/// One call's locals.
pub struct Frame<'b> {
    body: &'b Body,
    locals: IndexVec<Local, Option<Value>>,
}

impl<'b> Frame<'b> {
    /// A frame for `body` with `args` in its parameters.
    ///
    /// # Errors
    /// Returns [`Unevaluated::Unsupported`] when the arguments do not match the parameters.
    pub fn new(body: &'b Body, args: &[Value]) -> Result<Self, Unevaluated> {
        if args.len() != body.params.len() {
            return Err(Unevaluated::Unsupported);
        }
        let mut locals = IndexVec::filled(None, body.locals.len());
        for (&param, arg) in body.params.iter().zip(args) {
            locals[param] = Some(arg.clone());
        }
        Ok(Self { body, locals })
    }

    /// Runs the body to its return, and gives the result.
    ///
    /// # Errors
    /// Returns why a statement has no value.
    pub fn run(mut self, cx: &Cx<'_>) -> Result<Value, Unevaluated> {
        let mut block = &self.body.blocks[overt_index::Idx::new(0)];
        loop {
            for stmt in &block.stmts {
                let value = self.rvalue(cx, &stmt.value)?;
                self.locals[stmt.place.local] = Some(value);
            }
            match block.term.kind {
                TerminatorKind::Return => {
                    return self.locals[self.body.ret].take().ok_or(Unevaluated::Unsupported);
                }
                TerminatorKind::Goto(to) => block = &self.body.blocks[to],
            }
        }
    }

    fn operand(&self, operand: &Operand) -> Result<Value, Unevaluated> {
        match operand {
            Operand::Place(place) => self.locals[place.local].clone().ok_or(Unevaluated::Unsupported),
            Operand::Const(value) => Ok(value.clone()),
        }
    }

    fn rvalue(&self, cx: &Cx<'_>, rvalue: &Rvalue) -> Result<Value, Unevaluated> {
        match rvalue {
            Rvalue::Use(operand) => self.operand(operand),
            Rvalue::Binary { op, left, right } => {
                Value::binary(*op, &self.operand(left)?, &self.operand(right)?)
            }
            Rvalue::Unary { op, operand } => Value::unary(*op, &self.operand(operand)?),
            Rvalue::Range { op, lo, hi } => {
                let hi = hi.as_ref().map(|h| self.operand(h)).transpose()?;
                Value::range(*op, &self.operand(lo)?, hi.as_ref())
            }
            Rvalue::Call { callee, args } => {
                let args = args.iter().map(|a| self.operand(a)).collect::<Result<Vec<_>, _>>()?;
                match self.operand(callee)? {
                    Value::Fn(f) => cx.get::<Call>(&(f, args)),
                    Value::Primitive(p) => p.apply(&args),
                    _ => Err(Unevaluated::Unsupported),
                }
            }
        }
    }
}
