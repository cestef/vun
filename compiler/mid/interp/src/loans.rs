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
use vun_sema::eval::{Metered, MeteredCall};
use vun_sema_ty::Value;
use vun_sema_ty::call::Instance;
use vun_sema_ty::ops::Part;
use vun_sema_ty::trap::Unevaluated;

use crate::machine::Frame;

/// A loan passed to a call: the argument's position, and the place it reaches here.
struct Lent {
    position: usize,
    local: Local,
    parts: Vec<Part>,
}

/// Loans inside compile-time values.
pub(crate) trait Loans: Sized {
    /// Whether this holds a loan inside it, where a call could not give it back.
    fn holds_loan(&self) -> bool;

    /// This value with each loan of a local from `from` on renumbered to start at `to`: the
    /// storage lent to a call follows its arguments, and its body's locals precede it.
    #[must_use]
    fn relocate(self, from: usize, to: usize) -> Self;
}

impl Loans for Value {
    fn holds_loan(&self) -> bool {
        match self {
            Self::Loan { .. } => true,
            Self::Tuple(items) | Self::Array(items) => items.iter().any(Self::holds_loan),
            Self::Variant { payload, .. } => payload.iter().any(Self::holds_loan),
            Self::Struct { fields, .. } => fields.iter().any(|(_, v)| v.holds_loan()),
            Self::Closure { captures, .. } => captures.iter().any(Self::holds_loan),
            _ => false,
        }
    }

    fn relocate(self, from: usize, to: usize) -> Self {
        let each = |items: Vec<Self>| items.into_iter().map(|v| v.relocate(from, to)).collect();
        match self {
            Self::Loan { local, parts } if local >= from => Self::Loan { local: local - from + to, parts },
            Self::Tuple(items) => Self::Tuple(each(items)),
            Self::Array(items) => Self::Array(each(items)),
            Self::Variant { ty, name, payload } => Self::Variant { ty, name, payload: each(payload) },
            Self::Struct { ty, fields } => Self::Struct {
                ty,
                fields: fields.into_iter().map(|(name, v)| (name, v.relocate(from, to))).collect(),
            },
            Self::Closure { f, captures } => Self::Closure { f, captures: each(captures) },
            value => value,
        }
    }
}

/// What the loans nested in a call's arguments reach, which follows the arguments as storage the
/// callee holds, and the loans each came from.
struct Lending {
    /// How many arguments the call has: the storage starts after them.
    arguments: usize,
    storage: Vec<Value>,
    lent: Vec<Lent>,
}

impl Frame<'_, '_, '_> {
    /// Whether `local` is typed as an exclusive borrow. One holding a value rather than a loan is
    /// a parameter lent to this call, which holds what its caller's loan reaches.
    pub(crate) fn exclusive(&self, local: Local) -> bool {
        self.body
            .locals
            .get(local)
            .and_then(|l| l.ty)
            .and_then(|t| t.borrowing())
            .is_some_and(|(mode, ..)| mode.access() == Access::Exclusive)
    }

    /// `value`, an argument or a part of one, with each loan in it, nested in an aggregate, taken
    /// from the call's arguments: what it reaches is lent as storage after them, and the value
    /// holds a loan of that storage, which the callee follows as it follows any loan.
    fn lend(&self, value: Value, lending: &mut Lending) -> Result<Value, Unevaluated> {
        Ok(match value {
            Value::Loan { local, parts } => {
                let local = self.lent(local)?;
                let reached = self.at(local, &parts)?;
                if reached.holds_loan() {
                    return Err(Unevaluated::Unsupported);
                }
                let position = lending.arguments + lending.storage.len();
                lending.storage.push(reached);
                lending.lent.push(Lent { position, local, parts });
                Value::Loan { local: position, parts: Vec::new() }
            }
            Value::Tuple(items) => Value::Tuple(self.lend_each(items, lending)?),
            Value::Array(items) => Value::Array(self.lend_each(items, lending)?),
            Value::Variant { ty, name, payload } => {
                Value::Variant { ty, name, payload: self.lend_each(payload, lending)? }
            }
            Value::Struct { ty, fields } => {
                let (names, values): (Vec<_>, Vec<_>) = fields.into_iter().unzip();
                Value::Struct {
                    ty,
                    fields: names.into_iter().zip(self.lend_each(values, lending)?).collect(),
                }
            }
            // A closure holding a loan never leaves the frame that made it.
            value if value.holds_loan() => return Err(Unevaluated::Unsupported),
            value => value,
        })
    }

    fn lend_each(&self, items: Vec<Value>, lending: &mut Lending) -> Result<Vec<Value>, Unevaluated> {
        items.into_iter().map(|v| self.lend(v, lending)).collect()
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
    /// exclusive parameters hold when it returns. A closure passes its captures first, its loans
    /// among them: a closure holding one never leaves the frame that made it.
    #[inline(never)]
    pub(crate) fn call(&mut self, callee: &Operand, args: &[Operand]) -> Result<Value, Unevaluated> {
        let callee = self.operand(callee)?;
        let (f, captures) = match &callee {
            Value::Fn(f) | Value::Interface(f) => (Some(f.clone()), Vec::new()),
            Value::Closure { f, captures } => (Some(f.clone()), captures.clone()),
            _ => (None, Vec::new()),
        };
        let given = args.iter().map(|arg| self.operand(arg)).collect::<Result<Vec<_>, _>>()?;
        let arguments = captures.len() + given.len();
        let mut lending = Lending { arguments, storage: Vec::new(), lent: Vec::new() };
        let mut values = Vec::with_capacity(arguments);
        for (position, value) in captures.into_iter().chain(given).enumerate() {
            let value = match value {
                Value::Loan { local, parts } => {
                    let local = self.lent(local)?;
                    let reached = self.at(local, &parts)?;
                    lending.lent.push(Lent { position, local, parts });
                    reached
                }
                value => self.lend(value, &mut lending)?,
            };
            values.push(value);
        }
        let Lending { storage, lent, .. } = lending;
        values.extend(storage);
        // A call of a function counts the steps it took (spec 03, section 7); a primitive changes
        // nothing it is lent.
        let Some(f) = f else {
            return if lent.is_empty() {
                self.env.apply(callee, values)
            } else {
                Err(Unevaluated::Unsupported)
            };
        };
        let outcome = self.env.cx.get::<MeteredCall>(&Instance { f, args: values });
        self.spend(outcome.steps)?;
        let result = outcome.value?;
        // A frame holding what it was lent would change it after this call returns.
        if !lent.is_empty() && matches!(result, Value::Frame(_)) {
            return Err(Unevaluated::Unsupported);
        }
        for loan in &lent {
            if let Some(Some(value)) = outcome.written.get(loan.position) {
                self.store(loan.local, &loan.parts, value.clone())?;
            }
        }
        match result {
            Value::Loan { local: position, parts } => {
                let loan = lent.iter().find(|l| l.position == position).ok_or(Unevaluated::Unsupported)?;
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
        let own = self.body.locals.len();
        let params = self.body.params.len();
        let lent = (own..self.locals.len()).map(|at| self.locals[Local::new(at)].clone());
        let written = self
            .body
            .params
            .iter()
            .map(|&p| self.exclusive(p).then(|| self.locals[p].clone()).flatten())
            .chain(lent)
            .collect::<Vec<_>>();
        // What a parameter is left holding goes back to the caller, where a loan of this call's
        // storage would name another frame's.
        let escapes = written.iter().flatten().any(Value::holds_loan);
        let value = value.and_then(|value| match value {
            _ if escapes => Err(Unevaluated::Unsupported),
            // The position of the argument or the storage lent after the arguments it reaches.
            Value::Loan { local, parts } => {
                let position = self.body.params.iter().position(|p| p.index() == local);
                let lent = local.checked_sub(own).map(|at| params + at).filter(|&at| at < written.len());
                position.or(lent).map(|local| Value::Loan { local, parts }).ok_or(Unevaluated::Unsupported)
            }
            value if value.holds_loan() => Err(Unevaluated::Unsupported),
            value => Ok(value),
        });
        Metered { value, steps: self.steps, written }
    }
}
