//! Expressions run.

use std::cmp::Ordering;

use super::stmt::Flow;
use super::{Frame, Machine, Trap};
use crate::float::Float;
use crate::int::BigInt;
use std::cell::RefCell;
use std::rc::Rc;

use vun_index::Idx;

use crate::ir::{Arith, Capturing, Compare, Convert, Expr, FnId, Kind, Logic, Narrow, Pattern, Unary};
use crate::ty::{Nominal, Ty};
use crate::value::{Cell, Value};

impl Machine<'_> {
    /// The value of `expr` in `frame`, of the function `within`.
    #[allow(clippy::too_many_lines, reason = "one arm per form, as declared")]
    pub(super) fn eval(&mut self, expr: &Expr, frame: &mut Frame, within: FnId) -> Result<Value, Trap> {
        self.step()?;
        Ok(match &expr.kind {
            Kind::Lit(value) => value.clone(),
            Kind::Local(id) => frame.get(*id)?,
            Kind::Closure(closure) => {
                let mut held = Vec::new();
                for capture in &closure.captures {
                    let cell = match capture.how {
                        Capturing::Copy => Cell(Rc::new(RefCell::new(frame.get(capture.local)?))),
                        Capturing::Shared | Capturing::Mut => frame.cell(capture.local)?,
                    };
                    held.push((capture.local, cell));
                }
                Value::Closure(Rc::new((**closure).clone()), within, held)
            }
            Kind::CallClosure(callee, args) => {
                let Value::Closure(closure, written, held) = self.eval(callee, frame, within)? else {
                    return Err(Trap::Shape);
                };
                let args = self.all(args, frame, within)?;
                // Its locals are those of the function that wrote it, whoever calls it.
                let mut inner = Frame::new(self.program.functions[written].locals.len());
                for (local, cell) in held {
                    inner.locals[local.index()] = Some(cell);
                }
                for (local, arg) in closure.params.iter().zip(args) {
                    inner.bind(*local, arg);
                }
                self.eval(&closure.body, &mut inner, written)?
            }
            Kind::Borrow(_, place) => {
                let cell = frame.cell(place.local)?;
                if place.deref {
                    let Value::Ref(held) = cell.0.borrow().clone() else { return Err(Trap::Shape) };
                    Value::Ref(held)
                } else {
                    Value::Ref(cell)
                }
            }
            Kind::Deref(reference) => {
                let Value::Ref(cell) = self.eval(reference, frame, within)? else { return Err(Trap::Shape) };

                cell.0.borrow().clone()
            }
            Kind::Unary(op, operand) => {
                let value = self.eval(operand, frame, within)?;
                Self::unary(*op, &value, &operand.ty)?
            }
            Kind::Arith(op, left, right) if matches!(left.ty, Ty::Float(_)) => {
                let (a, b) = (self.eval(left, frame, within)?, self.eval(right, frame, within)?);
                let (Value::Float(a), Value::Float(b), Ty::Float(format)) = (a, b, &left.ty) else {
                    return Err(Trap::Shape);
                };
                Value::Float(Float::arith(*format, *op, a, b).ok_or(Trap::Shape)?)
            }
            Kind::Compare(op, left, right) if matches!(left.ty, Ty::Float(_)) => {
                let (a, b) = (self.eval(left, frame, within)?, self.eval(right, frame, within)?);
                let (Value::Float(a), Value::Float(b)) = (a, b) else { return Err(Trap::Shape) };
                Value::Bool(Float::compare(*op, a, b))
            }
            Kind::Convert(how, operand) => {
                let value = self.eval(operand, frame, within)?;
                match (how, &expr.ty, value) {
                    (Convert::Nearest, Ty::Float(to), Value::Int(n)) => {
                        Value::Float(Float::from_int(*to, &n))
                    }
                    (Convert::Nearest, Ty::Float(to), Value::Float(x)) => Value::Float(x.converted(*to)),
                    (Convert::Truncate, Ty::Int(range), Value::Float(x)) => Value::Int(x.truncated(range)),
                    _ => return Err(Trap::Shape),
                }
            }
            Kind::Arith(op, left, right) => {
                let (a, b) = (self.eval(left, frame, within)?, self.eval(right, frame, within)?);
                Self::arith(*op, a.int().ok_or(Trap::Shape)?, b.int().ok_or(Trap::Shape)?, &left.ty)?
            }
            Kind::Compare(op, left, right) => {
                let (a, b) = (self.eval(left, frame, within)?, self.eval(right, frame, within)?);
                let order = a.int().ok_or(Trap::Shape)?.cmp(b.int().ok_or(Trap::Shape)?);
                Value::Bool(match op {
                    Compare::Eq => order == Ordering::Equal,
                    Compare::Ne => order != Ordering::Equal,
                    Compare::Lt => order == Ordering::Less,
                    Compare::Le => order != Ordering::Greater,
                    Compare::Gt => order == Ordering::Greater,
                    Compare::Ge => order != Ordering::Less,
                })
            }
            Kind::Logic(op, left, right) => {
                let Value::Bool(a) = self.eval(left, frame, within)? else { return Err(Trap::Shape) };
                match (op, a) {
                    (Logic::And, false) | (Logic::Or, true) => Value::Bool(a),
                    _ => self.eval(right, frame, within)?,
                }
            }
            Kind::Narrow(how, operand) => {
                let value = self.eval(operand, frame, within)?;
                let n = value.int().ok_or(Trap::Shape)?;
                let to = expr.ty.range().ok_or(Trap::Shape)?;
                Value::Int(match how {
                    Narrow::Wrap => to.wrap(n),
                    Narrow::Sat => to.saturate(n),
                    Narrow::To if to.contains(n) => n.clone(),
                    Narrow::To => return Err(Trap::Narrowed(n.clone(), to.clone())),
                })
            }
            Kind::If(condition, then, otherwise) => match self.eval(condition, frame, within)? {
                Value::Bool(true) => self.eval(then, frame, within)?,
                Value::Bool(false) => self.eval(otherwise, frame, within)?,
                _ => return Err(Trap::Shape),
            },
            Kind::Tuple(items) => Value::Tuple(self.all(items, frame, within)?),
            Kind::Array(items) => Value::Array(self.all(items, frame, within)?),
            Kind::Repeat(item, count) => {
                let value = self.eval(item, frame, within)?;
                Value::Array(vec![value; *count])
            }
            Kind::Field(tuple, at) => {
                let value = self.eval(tuple, frame, within)?;
                value.parts().and_then(|p| p.get(*at)).cloned().ok_or(Trap::Shape)?
            }
            Kind::Index(array, index) => {
                let (array, index) = (self.eval(array, frame, within)?, self.eval(index, frame, within)?);
                let items = array.parts().ok_or(Trap::Shape)?;
                let n = index.int().ok_or(Trap::Shape)?;
                let at = usize::try_from(n).ok().filter(|&at| at < items.len());
                items[at.ok_or_else(|| Trap::Index(n.clone(), items.len()))?].clone()
            }
            Kind::Call(function, _, args) => {
                let args = self.all(args, frame, within)?;
                self.call(*function, args)?
            }
            Kind::Block(stmts, tail) => match self.scope(stmts, Some(tail), frame, within)? {
                (Flow::Next, Some(value)) => value,
                _ => return Err(Trap::Escape),
            },
            Kind::TailCall(function, args) => {
                let args = self.all(args, frame, within)?;
                self.call(*function, args)?
            }
            Kind::Forever(label, stmts) => self.forever(*label, stmts, frame, within)?,
            Kind::TryTo(operand) => {
                let value = self.eval(operand, frame, within)?;
                let n = value.int().ok_or(Trap::Shape)?;
                let Ty::Enum(_, variants) = &expr.ty else { return Err(Trap::Shape) };
                let Ty::Int(to) = &variants[1][0] else { return Err(Trap::Shape) };
                if to.contains(n) { Value::Variant(1, vec![value]) } else { Value::Variant(0, Vec::new()) }
            }
            Kind::UnwrapOr(option, default) => {
                let Value::Variant(held, mut payload) = self.eval(option, frame, within)? else {
                    return Err(Trap::Shape);
                };
                let default = self.eval(default, frame, within)?;
                if held == 1 { payload.pop().ok_or(Trap::Shape)? } else { default }
            }
            Kind::Try(operand) => {
                let Value::Variant(held, mut payload) = self.eval(operand, frame, within)? else {
                    return Err(Trap::Shape);
                };
                let Ty::Enum(nominal, _) = &operand.ty else { return Err(Trap::Shape) };
                let goes_on = usize::from(*nominal == Nominal::Option);
                if held != goes_on {
                    return Err(Trap::Return(Value::Variant(held, payload)));
                }
                payload.pop().ok_or(Trap::Shape)?
            }
            Kind::IsSome(option) => {
                let Value::Variant(held, _) = self.eval(option, frame, within)? else {
                    return Err(Trap::Shape);
                };
                Value::Bool(held == 1)
            }
            Kind::Struct(_, fields) => Value::Tuple(self.all(fields, frame, within)?),
            Kind::Variant(_, variant, payload) => Value::Variant(*variant, self.all(payload, frame, within)?),
            Kind::Match(scrutinee, arms) => {
                let Value::Variant(held, payload) = self.eval(scrutinee, frame, within)? else {
                    return Err(Trap::Shape);
                };
                for arm in arms {
                    match &arm.pattern {
                        Pattern::Variant { variant, binds } if *variant == held => {
                            for (bind, part) in binds.iter().zip(payload) {
                                if let Some(local) = bind {
                                    frame.bind(*local, part);
                                }
                            }
                            return self.eval(&arm.body, frame, within);
                        }
                        Pattern::Variant { .. } => {}
                        Pattern::Any => return self.eval(&arm.body, frame, within),
                    }
                }
                return Err(Trap::Shape);
            }
        })
    }

    pub(super) fn all(
        &mut self,
        exprs: &[Expr],
        frame: &mut Frame,
        within: FnId,
    ) -> Result<Vec<Value>, Trap> {
        exprs.iter().map(|e| self.eval(e, frame, within)).collect()
    }

    fn unary(op: Unary, value: &Value, ty: &Ty) -> Result<Value, Trap> {
        Ok(match (op, value) {
            (Unary::Neg, Value::Int(n)) => Value::Int(-n),
            (Unary::Neg, Value::Float(x)) => Value::Float(x.negated()),
            (Unary::BitNot, Value::Int(n)) => {
                let word = ty.range().ok_or(Trap::Shape)?;
                Value::Int(&word.hi - n)
            }
            (Unary::Not, Value::Bool(b)) => Value::Bool(!b),
            _ => return Err(Trap::Shape),
        })
    }

    /// The result of `a op b`, `left` the type of `a`.
    pub(super) fn arith(op: Arith, a: &BigInt, b: &BigInt, left: &Ty) -> Result<Value, Trap> {
        let zero = BigInt::ZERO;
        let word = || left.range().ok_or(Trap::Shape);
        Ok(Value::Int(match op {
            Arith::Add => a + b,
            Arith::Sub => a - b,
            Arith::Mul => a * b,
            Arith::Div | Arith::Rem if *b == zero => return Err(Trap::Zero),
            Arith::Div => a / b,
            Arith::Rem => a % b,
            Arith::And => a & b,
            Arith::Or => a | b,
            Arith::Xor => a ^ b,
            Arith::Shl => {
                let shifts = u64::try_from(b).map_err(|_| Trap::Shape)?;
                word()?.wrap(&(a << shifts))
            }
            Arith::Shr => a >> u64::try_from(b).map_err(|_| Trap::Shape)?,
        }))
    }
}
