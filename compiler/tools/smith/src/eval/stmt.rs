//! Statements run.

use super::{Frame, Machine, Trap};
use crate::int::BigInt;
use crate::ir::{Arith, Assign, Expr, FnId, Kind, Label, Place, Stmt};
use crate::value::Value;

/// How a statement ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Flow {
    Next,
    Break(Option<Label>, Option<Value>),
    Continue(Option<Label>),
}

/// What a loop does with how its body ended.
enum Lap {
    Again,
    Leave(Option<Value>),
    /// The body left for a loop around this one.
    Pass(Flow),
}

impl Flow {
    /// What the loop named `mine` does about it.
    fn lap(self, mine: Option<Label>) -> Lap {
        match self {
            Self::Next => Lap::Again,
            Self::Continue(label) if label.is_none() || label == mine => Lap::Again,
            Self::Break(label, value) if label.is_none() || label == mine => Lap::Leave(value),
            passed => Lap::Pass(passed),
        }
    }
}

impl Machine<'_> {
    /// The statements in order and then the tail, up to a `break` or `continue`, with the
    /// statements deferred in them run at the end, the latest first, whichever way it comes: after
    /// the tail has its value, and before a tail call jumps.
    pub(super) fn scope<'s>(
        &mut self,
        stmts: &'s [Stmt],
        tail: Option<&Expr>,
        frame: &mut Frame,
        within: FnId,
    ) -> Result<(Flow, Option<Value>), Trap> {
        let mut deferred: Vec<&'s Stmt> = Vec::new();
        let mut ended = self.sequence(stmts, &mut deferred, frame, within);
        let (mut value, mut jump) = (None, None);
        if let (Ok(Flow::Next), Some(tail)) = (&ended, tail) {
            let reached = match &tail.kind {
                Kind::TailCall(function, args) => {
                    self.all(args, frame, within).map(|args| jump = Some((*function, args)))
                }
                _ => self.eval(tail, frame, within).map(|v| value = Some(v)),
            };
            if let Err(trap) = reached {
                ended = Err(trap);
            }
        }
        if !matches!(ended, Err(Trap::Fuel)) {
            for stmt in deferred.into_iter().rev() {
                if let Err(trap) = self.stmt(stmt, None, frame, within) {
                    ended = ended.and(Err(trap));
                }
            }
        }
        let flow = ended?;
        if let Some((function, args)) = jump {
            value = Some(self.call(function, args)?);
        }
        Ok((flow, value))
    }

    /// The statements in order, the deferred ones set aside as they are reached.
    fn sequence<'s>(
        &mut self,
        stmts: &'s [Stmt],
        deferred: &mut Vec<&'s Stmt>,
        frame: &mut Frame,
        within: FnId,
    ) -> Result<Flow, Trap> {
        for stmt in stmts {
            self.step()?;
            if let Stmt::Defer(inner) = stmt {
                deferred.push(inner);
                continue;
            }
            match self.stmt(stmt, None, frame, within)? {
                Flow::Next => {}
                escaped => return Ok(escaped),
            }
        }
        Ok(Flow::Next)
    }

    /// What `function` yields for `args`, all of it.
    fn yields(&mut self, function: FnId, args: Vec<Value>) -> Result<Vec<Value>, Trap> {
        self.yielded.push(Vec::new());
        let ran = self.call(function, args);
        let items = self.yielded.pop().unwrap_or_default();
        ran.map(|_| items)
    }

    /// The statements of a loop forever, up to the `break` that leaves it: its value.
    pub(super) fn forever(
        &mut self,
        label: Label,
        stmts: &[Stmt],
        frame: &mut Frame,
        within: FnId,
    ) -> Result<Value, Trap> {
        loop {
            let flow = self.block(stmts, frame, within)?;
            match flow.lap(Some(label)) {
                Lap::Again => {}
                Lap::Leave(value) => return value.ok_or(Trap::Shape),
                Lap::Pass(_) => return Err(Trap::Escape),
            }
        }
    }

    fn block(&mut self, stmts: &[Stmt], frame: &mut Frame, within: FnId) -> Result<Flow, Trap> {
        self.scope(stmts, None, frame, within).map(|(flow, _)| flow)
    }

    /// `stmt`, a loop named `label` if it has a name.
    fn stmt(
        &mut self,
        stmt: &Stmt,
        label: Option<Label>,
        frame: &mut Frame,
        within: FnId,
    ) -> Result<Flow, Trap> {
        match stmt {
            Stmt::Let { local, init, .. } => {
                let value = self.eval(init, frame, within)?;
                frame.bind(*local, value);
            }
            Stmt::Assign { place, op, value } => {
                let value = self.eval(value, frame, within)?;
                self.assign(place, *op, &value, frame, within)?;
            }
            Stmt::If { condition, then, otherwise } => {
                let Value::Bool(chosen) = self.eval(condition, frame, within)? else {
                    return Err(Trap::Shape);
                };
                return self.block(if chosen { then } else { otherwise }, frame, within);
            }
            Stmt::Count { local, count, body } => {
                for i in 0..*count {
                    frame.bind(*local, Value::Int(BigInt::from(i)));
                    if let Some(flow) = self.lap(body, label, frame, within)? {
                        return Ok(flow);
                    }
                }
            }
            Stmt::Each { local, array, body } => {
                let held = frame.get(*array)?;
                for item in held.parts().ok_or(Trap::Shape)? {
                    frame.bind(*local, item.clone());
                    if let Some(flow) = self.lap(body, label, frame, within)? {
                        return Ok(flow);
                    }
                }
            }
            Stmt::While { condition, body } => loop {
                let Value::Bool(holds) = self.eval(condition, frame, within)? else {
                    return Err(Trap::Shape);
                };
                if !holds {
                    break;
                }
                self.step()?;
                if let Some(flow) = self.lap(body, label, frame, within)? {
                    return Ok(flow);
                }
            },
            Stmt::WhileSome { local, option, body } => loop {
                let Value::Variant(held, mut payload) = self.eval(option, frame, within)? else {
                    return Err(Trap::Shape);
                };
                if held != 1 {
                    break;
                }
                frame.bind(*local, payload.pop().ok_or(Trap::Shape)?);
                self.step()?;
                if let Some(flow) = self.lap(body, label, frame, within)? {
                    return Ok(flow);
                }
            },
            Stmt::Drain { local, function, args, body } => {
                let args = self.all(args, frame, within)?;
                for item in self.yields(*function, args)? {
                    frame.bind(*local, item);
                    if let Some(flow) = self.lap(body, label, frame, within)? {
                        return Ok(flow);
                    }
                }
            }
            Stmt::Yield(value) => {
                let value = self.eval(value, frame, within)?;
                self.yielded.last_mut().ok_or(Trap::Shape)?.push(value);
            }
            Stmt::Defer(_) => return Err(Trap::Shape),
            Stmt::Labeled(named, inner) => return self.stmt(inner, Some(*named), frame, within),
            Stmt::Break(target, value) => {
                let value = value.as_ref().map(|v| self.eval(v, frame, within)).transpose()?;
                return Ok(Flow::Break(*target, value));
            }
            Stmt::Continue(target) => return Ok(Flow::Continue(*target)),
        }
        Ok(Flow::Next)
    }

    /// One run of a loop's body: how the loop goes on, as `None` while it does.
    fn lap(
        &mut self,
        body: &[Stmt],
        label: Option<Label>,
        frame: &mut Frame,
        within: FnId,
    ) -> Result<Option<Flow>, Trap> {
        Ok(match self.block(body, frame, within)?.lap(label) {
            Lap::Again => None,
            Lap::Leave(_) => Some(Flow::Next),
            Lap::Pass(flow) => Some(flow),
        })
    }

    /// `place op= value` (spec 04, rule 9): a plain store, or the operation wrapped or clamped to the place's type.
    fn assign(
        &mut self,
        place: &Place,
        op: Assign,
        value: &Value,
        frame: &mut Frame,
        within: FnId,
    ) -> Result<(), Trap> {
        let function = &self.program.functions[within];
        let root = &function.locals[place.local].ty;
        let (_, ty) =
            Place::new(place.local, root, place.deref, place.steps.clone()).map_err(|_| Trap::Shape)?;
        self.at_place(place, frame, within, |held| {
            *held = match op.arith() {
                None => value.clone(),
                Some(arith) => {
                    let range = ty.range().ok_or(Trap::Shape)?;
                    let (a, b) = (held.int().ok_or(Trap::Shape)?, value.int().ok_or(Trap::Shape)?);
                    let Value::Int(exact) = Machine::arith(arith, a, b, &ty)? else {
                        return Err(Trap::Shape);
                    };
                    Value::Int(if op.saturates() { range.saturate(&exact) } else { range.wrap(&exact) })
                }
            };
            Ok(())
        })
    }
}

impl Assign {
    /// The arithmetic the assignment does of the place and the value, if any.
    pub(crate) const fn arith(self) -> Option<Arith> {
        match self {
            Self::Set => None,
            Self::AddWrap | Self::AddSat => Some(Arith::Add),
            Self::SubWrap | Self::SubSat => Some(Arith::Sub),
            Self::MulWrap | Self::MulSat => Some(Arith::Mul),
        }
    }

    /// Whether the result is clamped to the type, not wrapped into it.
    pub(crate) const fn saturates(self) -> bool {
        matches!(self, Self::AddSat | Self::SubSat | Self::MulSat)
    }
}
