//! Exclusive borrows at compile time (spec 03, section 7). `&mut x` and the other exclusive modes
//! of the table `borrows` make a loan of the place, which `.*` follows. A call takes what a loan
//! reaches in place of the loan and gives back what its exclusive parameters hold when it
//! returns, which the caller stores through the loan: exclusivity makes that the same as sharing
//! the place, and each call stays a pure function of its arguments, memoized as any other. A
//! borrow a call returns into a parameter comes back as a loan of that parameter, which the
//! caller rebases on its own.

use vun_desugar::{Access, Borrow};
use vun_index::Idx;
use vun_mir::{Local, Operand, Place};
use vun_sema::env::Unevaluated;
use vun_sema::eval::{Instance, Metered, MeteredCall};
use vun_sema::ops::Part;
use vun_sema::ty::Value;

use crate::machine::Frame;

/// A loan passed to a call: the argument's position, and the place it reaches here.
struct Lent {
    position: usize,
    local: Local,
    parts: Vec<Part>,
}

/// Whether `value` holds a loan inside it, where a call could not give it back.
fn holds_loan(value: &Value) -> bool {
    match value {
        Value::Loan { .. } => true,
        Value::Tuple(items) | Value::Array(items) => items.iter().any(holds_loan),
        Value::Variant { payload, .. } => payload.iter().any(holds_loan),
        Value::Struct { fields, .. } => fields.iter().any(|(_, v)| holds_loan(v)),
        Value::Closure { captures, .. } => captures.iter().any(holds_loan),
        _ => false,
    }
}

impl Frame<'_, '_, '_> {
    /// Whether `local` is typed as an exclusive borrow. One holding a value rather than a loan is
    /// a parameter lent to this call, which holds what its caller's loan reaches.
    pub(crate) fn exclusive(&self, local: Local) -> bool {
        self.body.locals[local]
            .ty
            .and_then(|t| t.borrowing())
            .is_some_and(|(mode, ..)| mode.access() == Access::Exclusive)
    }

    /// `&mode place`: a loan of the place for an exclusive mode; the value for a shared one,
    /// which a shared borrow of a compile-time value reads as.
    pub(crate) fn borrow(&self, mode: Borrow, place: &Place) -> Result<Value, Unevaluated> {
        if mode.access() != Access::Exclusive {
            return self.read(place);
        }
        let (local, parts) = self.resolve(place)?;
        Ok(Value::Loan { local: local.index(), parts })
    }

    /// `callee(args)`: loans are passed as what they reach, and get back what the callee's
    /// exclusive parameters hold when it returns.
    pub(crate) fn call(&mut self, callee: &Operand, args: &[Operand]) -> Result<Value, Unevaluated> {
        let mut lent = Vec::new();
        let mut values = Vec::with_capacity(args.len());
        for (position, arg) in args.iter().enumerate() {
            let value = match self.operand(arg)? {
                Value::Loan { local, parts } => {
                    let local = self.lent(local)?;
                    let reached = self.at(local, &parts)?;
                    lent.push(Lent { position, local, parts });
                    reached
                }
                value if holds_loan(&value) => return Err(Unevaluated::Unsupported),
                value => value,
            };
            values.push(value);
        }
        // A call of a function counts the steps it took (spec 03, section 7); a closure passes its
        // captures first.
        let (f, captured, values) = match self.operand(callee)? {
            Value::Fn(f) | Value::Interface(f) => (f, 0, values),
            Value::Closure { f, captures } => {
                (f, captures.len(), captures.into_iter().chain(values).collect())
            }
            // A primitive changes nothing it is lent.
            callee if lent.is_empty() => return self.env.apply(callee, values),
            _ => return Err(Unevaluated::Unsupported),
        };
        let outcome = self.env.cx.get::<MeteredCall>(&Instance { f, args: values });
        self.spend(outcome.steps)?;
        let result = outcome.value?;
        // A frame holding what it was lent would change it after this call returns.
        if !lent.is_empty() && matches!(result, Value::Frame(_)) {
            return Err(Unevaluated::Unsupported);
        }
        for loan in &lent {
            if let Some(Some(value)) = outcome.written.get(captured + loan.position) {
                self.store(loan.local, &loan.parts, value.clone())?;
            }
        }
        match result {
            Value::Loan { local: position, parts } => {
                let loan = lent
                    .iter()
                    .find(|l| captured + l.position == position)
                    .ok_or(Unevaluated::Unsupported)?;
                let parts = loan.parts.iter().cloned().chain(parts).collect();
                Ok(Value::Loan { local: loan.local.index(), parts })
            }
            result => Ok(result),
        }
    }

    /// The outcome of a run returning `value`: what each exclusive parameter holds, and a loan
    /// in the result named by the position of the parameter it was lent through. A loan of the
    /// call's own storage cannot outlive it.
    pub(crate) fn finish(self, value: Result<Value, Unevaluated>) -> Metered {
        let written = self
            .body
            .params
            .iter()
            .map(|&p| self.exclusive(p).then(|| self.locals[p].clone()).flatten())
            .collect();
        let value = value.and_then(|value| match value {
            Value::Loan { local, parts } => {
                let position = self.body.params.iter().position(|p| p.index() == local);
                position.map(|local| Value::Loan { local, parts }).ok_or(Unevaluated::Unsupported)
            }
            value if holds_loan(&value) => Err(Unevaluated::Unsupported),
            value => Ok(value),
        });
        Metered { value, steps: self.steps, written }
    }
}
