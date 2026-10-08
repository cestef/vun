//! Loops that end by themselves: a counter bounds each, and a `break` may leave it sooner.

use super::Draw;
use super::scope::Exit;
use crate::int::Range;
use crate::ir::{Assign, Binding, Compare, Expr, Label, LocalId, Logic, Place, Stmt};
use crate::ty::Ty;

impl Draw<'_> {
    /// How many times a loop runs at most.
    pub(super) fn times(&mut self) -> u32 {
        u32::try_from(self.entropy.between(1, self.config.iterations as usize)).unwrap_or(1)
    }

    /// A counter, `mut n: u8 = 0`, for a loop the body of which steps it with [`Self::tick`].
    fn counter(&mut self) -> (LocalId, Stmt) {
        let local = self.scope.bind(Ty::Int(Range::new(0, 255)), true);
        (local, Stmt::Let { local, init: Expr::int(0), binding: Binding::Annotated })
    }

    /// `n +%= 1`
    fn tick(&self, counter: LocalId) -> Stmt {
        let (place, _) = Place::new(counter, &self.scope.locals[counter].ty, false, Vec::new())
            .expect("a counter is a place");
        Stmt::Assign { place, op: Assign::AddWrap, value: Expr::int(1) }
    }

    /// `n < limit`
    fn below_limit(&self, counter: LocalId, limit: u32) -> Expr {
        let n = Expr::local(counter, &self.scope.locals[counter].ty);
        Expr::compare(Compare::Lt, n, Expr::int(limit)).expect("a counter is compared")
    }

    /// `loop` named `label`, if it has a name.
    pub(super) fn named(label: Option<Label>, stmt: Stmt) -> Stmt {
        match label {
            Some(label) => Stmt::Labeled(label, Box::new(stmt)),
            None => stmt,
        }
    }

    /// `for n < limit && condition { n +%= 1; ... }`, after the counter's binding.
    pub(super) fn while_loop(&mut self, label: Option<Label>) -> Option<Vec<Stmt>> {
        let limit = self.times();
        let seen = self.scope.visible.len();
        let (counter, init) = self.counter();
        self.scope.frozen.push(counter);
        let mut condition = self.below_limit(counter, limit);
        if self.entropy.chance(1, 2) {
            let runs = self.scope.runs;
            self.scope.runs = runs.saturating_mul(u64::from(limit) + 1);
            let more = self.bool(self.config.depth);
            self.scope.runs = runs;
            condition = Expr::logic(Logic::And, condition, more).ok()?;
        }
        let mut body = vec![self.tick(counter)];
        body.extend(self.looped(u64::from(limit), label));
        self.scope.frozen.pop();
        self.scope.visible.truncate(seen);
        Some(vec![init, Self::named(label, Stmt::While { condition, body })])
    }

    /// Whether a statement may be deferred here: not in an iterator or a closure.
    pub(super) fn defers(&self) -> bool {
        self.scope.yields.is_none() && !self.scope.inside_closure && !self.writable().is_empty()
    }

    /// The iterator functions made so far.
    pub(super) fn iterators(&self) -> impl Iterator<Item = crate::ir::FnId> + '_ {
        self.program.functions.iter().filter(|(_, f)| f.role == crate::ir::Role::Iterator).map(|(id, _)| id)
    }

    /// `for .some(v) = n.try_to(range) { n +%= 1; ... }`, after the counter's binding.
    pub(super) fn while_some(&mut self, label: Option<Label>) -> Option<Vec<Stmt>> {
        let limit = self.times();
        let seen = self.scope.visible.len();
        let (counter, init) = self.counter();
        self.scope.frozen.push(counter);
        let below = u32::try_from(self.entropy.between(0, 3)).unwrap_or(0);
        let range = Range::new(-i64::from(below), i64::from(limit) - 1);
        let operand = Expr::local(counter, &self.scope.locals[counter].ty);
        let option = Expr::try_to(operand, range.clone()).ok()?;
        let local = self.scope.bind(Ty::Int(range), false);
        let mut body = vec![self.tick(counter)];
        body.extend(self.looped(u64::from(limit), label));
        self.scope.frozen.pop();
        self.scope.visible.truncate(seen);
        Some(vec![init, Self::named(label, Stmt::WhileSome { local, option, body })])
    }

    /// `for v in iterator(args) { ... }`
    pub(super) fn drain(&mut self, label: Option<Label>) -> Option<Vec<Stmt>> {
        let candidates: Vec<crate::ir::FnId> = self
            .iterators()
            .filter(|&id| {
                let f = &self.program.functions[id];
                self.scope.steps.saturating_add(f.steps.saturating_mul(self.scope.runs)) < self.config.steps
            })
            .collect();
        if candidates.is_empty() {
            return None;
        }
        let function = *self.entropy.pick(&candidates);
        let (params, item, steps) = {
            let f = &self.program.functions[function];
            (f.signature(), f.result.clone(), f.steps)
        };
        let args = params.iter().map(|p| self.expr(p, self.config.depth)).collect();
        self.scope.steps = self.scope.steps.saturating_add(steps.saturating_mul(self.scope.runs));
        let seen = self.scope.visible.len();
        let local = self.scope.bind(item, false);
        let body = self.looped(steps.max(1), label);
        self.scope.visible.truncate(seen);
        Some(vec![Self::named(label, Stmt::Drain { local, function, args, body })])
    }

    /// `'label: for { n +%= 1; ...; if n < limit { continue; } break value }`, which gives `value`.
    pub(super) fn forever(&mut self, want: &Ty, depth: usize) -> Option<Expr> {
        if self.scope.nesting >= self.config.nesting
            || matches!(want, Ty::Unit | Ty::Ref(..) | Ty::Closure(..) | Ty::Param(_))
        {
            return None;
        }
        let limit = self.times();
        let label = self.scope.name_loop();
        let seen = self.scope.visible.len();
        let (counter, init) = self.counter();
        self.scope.frozen.push(counter);
        self.scope.exits.push(Exit { label, want: want.clone(), found: Vec::new() });
        let (loops, labels) = (std::mem::take(&mut self.scope.loops), std::mem::take(&mut self.scope.labels));
        let yields = self.scope.yields.take();
        let runs = self.scope.runs;
        self.scope.runs = runs.saturating_mul(u64::from(limit));

        let mut body = vec![self.tick(counter)];
        body.extend(self.block());
        let again = vec![Stmt::Continue(Some(label))];
        body.push(Stmt::If {
            condition: self.below_limit(counter, limit),
            then: again,
            otherwise: Vec::new(),
        });
        let value = self.expr(want, depth);
        let mut exit = self.scope.exits.pop().expect("the loop's exit is there");
        exit.found.push(value.ty.clone());
        body.push(Stmt::Break(None, Some(value)));

        self.scope.runs = runs;
        (self.scope.loops, self.scope.labels) = (loops, labels);
        self.scope.yields = yields;
        self.scope.frozen.pop();
        self.scope.visible.truncate(seen);
        let ty = exit.found.iter().try_fold(None::<Ty>, |held, ty| match held {
            None => Some(Some(ty.clone())),
            Some(held) => held.join(ty).map(Some),
        })??;
        ty.fits_a_word().then(|| Expr::block(vec![init], Expr::forever(label, body, ty)))
    }

    /// `if condition { break 'loop value }` out of the innermost loop forever.
    pub(super) fn exit(&mut self) -> Option<Stmt> {
        let (label, want) = self.scope.exits.last().map(|e| (e.label, e.want.clone()))?;
        let condition = self.bool(self.config.depth);
        let value = self.expr(&want, self.config.depth);
        self.scope.exits.last_mut()?.found.push(value.ty.clone());
        let label = (self.scope.loops > 0 || self.entropy.chance(1, 2)).then_some(label);
        Some(Stmt::If { condition, then: vec![Stmt::Break(label, Some(value))], otherwise: Vec::new() })
    }

    /// `if condition { break }` or `continue`, of the innermost loop or one named.
    pub(super) fn escape(&mut self) -> Stmt {
        let condition = self.bool(self.config.depth);
        let label = (!self.scope.labels.is_empty() && self.entropy.chance(1, 2))
            .then(|| *self.entropy.pick(&self.scope.labels.clone()));
        let escape = if self.entropy.chance(1, 2) { Stmt::Break(label, None) } else { Stmt::Continue(label) };
        Stmt::If { condition, then: vec![escape], otherwise: Vec::new() }
    }

    /// The body of a loop that runs `times` times.
    pub(super) fn looped(&mut self, times: u64, label: Option<Label>) -> Vec<Stmt> {
        let runs = self.scope.runs;
        self.scope.runs = runs.saturating_mul(times);
        self.scope.loops += 1;
        self.scope.labels.extend(label);
        let body = self.block();
        if label.is_some() {
            self.scope.labels.pop();
        }
        self.scope.loops -= 1;
        self.scope.runs = runs;
        body
    }
}
