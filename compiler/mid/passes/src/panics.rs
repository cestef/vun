//! `#[panic.never]` (spec 07, section 3): a function promising never to trap contains no trap
//! site its types do not discharge, and calls only functions making the same promise. Trap sites
//! here: a narrowing that may not fit (`Narrowing::To`), an index not proven within an array, and
//! a call of a function without the promise, where a primitive's compile-time arguments decide:
//! a division is one of `lang.int.div`, a trap site only by a divisor that may be 0.

use vun_desugar::Narrowing;
use vun_diag::{DiagCtxt, Diagnostic, Msg, errors};
use vun_mir::{Body, Mir, Operand, Place, Projection, Rvalue};
use vun_origin::OriginId;
use vun_query::{Cx, Cycle, Query};
use vun_sema::Apply;
use vun_sema::env::Env;
use vun_sema_int::{Extended, Integer, Interval};
use vun_sema_ty::call::Instantiation;
use vun_sema_ty::{Marker, TyKind, Value};

/// The `panic.never` reports of an instantiation's body; `None` when it has no MIR yet.
pub struct PanicChecks;

impl Query for PanicChecks {
    const NAME: &'static str = "panic_checks";
    type Key = Instantiation;
    type Value = Option<DiagCtxt>;

    fn compute(cx: &Cx<'_>, i: &Instantiation) -> Self::Value {
        let body = cx.get::<Mir>(i).ok()?;
        let env = Env::inside(cx, &i.f, i.args.iter().cloned())?;
        let mut check = Check { env: &env, body: &body, diag: DiagCtxt::new() };
        if Marker::never_traps(&env.literal_markers(i.f.literal.expr.id)) {
            check.run();
        }
        Some(check.diag)
    }
}

/// Whether an instantiation's body may trap: a trap site its types do not discharge, or a call of
/// what may. A closure is judged so where it is called, its promise being its body; a body without
/// MIR, or reached again while judged, may.
pub struct MayTrap;

impl Query for MayTrap {
    const NAME: &'static str = "may_trap";
    type Key = Instantiation;
    type Value = bool;

    fn compute(cx: &Cx<'_>, i: &Instantiation) -> Self::Value {
        let (Ok(body), Some(env)) = (cx.get::<Mir>(i), Env::inside(cx, &i.f, i.args.iter().cloned())) else {
            return true;
        };
        if Marker::never_traps(&env.literal_markers(i.f.literal.expr.id)) {
            return false;
        }
        let mut check = Check { env: &env, body: &body, diag: DiagCtxt::new() };
        check.run();
        check.diag.failed().is_some()
    }

    fn cycle(_: &Cx<'_>, _: &Instantiation, _: &Cycle) -> Self::Value {
        true
    }
}

struct Check<'e, 'a, 'db, 'b> {
    env: &'e Env<'a, 'db>,
    body: &'b Body,
    diag: DiagCtxt,
}

impl Check<'_, '_, '_, '_> {
    fn run(&mut self) {
        let body = self.body;
        for (_, block) in body.blocks.iter() {
            for stmt in &block.stmts {
                self.rvalue(&stmt.value, stmt.origin);
                self.indices(&stmt.place, stmt.origin);
            }
        }
    }

    fn rvalue(&mut self, rvalue: &Rvalue, origin: OriginId) {
        for operand in rvalue.operands() {
            if let Operand::Place(place) = operand {
                self.indices(place, origin);
            }
        }
        match rvalue {
            Rvalue::Call { callee, args } if self.may_trap(callee, args) => {
                self.report(
                    origin,
                    Msg::new().text("this call may trap: the function called is not ").code("#[panic.never]"),
                );
            }
            Rvalue::Narrow { operand, ty, narrowing: Narrowing::To } => {
                let fits = self.interval(operand).zip(ty.interval()).is_some_and(|(v, t)| t.contains(&v));
                if !fits {
                    self.report(
                        origin,
                        Msg::new().text("this narrows a value that may not fit, which traps"),
                    );
                }
            }
            Rvalue::Ref { place, .. }
            | Rvalue::IsVariant { place, .. }
            | Rvalue::Resume { frame: place, .. }
            | Rvalue::End { frame: place } => {
                self.indices(place, origin);
            }
            _ => {}
        }
    }

    /// Whether calling `callee` with `args` may trap: a closure whose body may ([`MayTrap`]), a
    /// function or primitive without the promise, but a primitive whose compile-time arguments show it cannot, such as a division by
    /// a divisor whose range excludes 0; a variant's constructor never does, and a callee of
    /// unknown type says nothing.
    fn may_trap(&self, callee: &Operand, args: &[Operand]) -> bool {
        let closure = match callee {
            Operand::Const(Value::Closure { f, .. }) => Some(f),
            Operand::Place(place) => match self.body.locals[place.local].ty.as_deref() {
                Some(TyKind::Closure(closure)) => Some(&closure.f),
                _ => None,
            },
            Operand::Const(_) => None,
        };
        if let Some(f) = closure {
            return self.env.cx.get::<MayTrap>(&Instantiation::new(f.clone(), Vec::new()));
        }
        if let Operand::Const(Value::Primitive(p)) = callee {
            let known: Vec<Option<Value>> = args
                .iter()
                .map(|a| match a {
                    Operand::Const(v) => Some(v.clone()),
                    Operand::Place(_) => None,
                })
                .collect();
            if let Some(traps) = p.may_trap(&known) {
                return traps;
            }
        }
        let markers = match callee {
            Operand::Const(Value::Fn(f)) => {
                let Some(env) = Env::inside(self.env.cx, f, []) else { return false };
                env.literal_markers(f.literal.expr.id)
            }
            Operand::Const(Value::Primitive(p)) => self.env.primitive_markers(*p),
            Operand::Place(place) => match self.body.locals[place.local].ty.and_then(|t| t.callable()) {
                Some(signature) => signature.markers,
                None => return false,
            },
            // A variant's constructor never traps; another constant is no function.
            Operand::Const(_) => return false,
        };
        !Marker::never_traps(&markers)
    }

    /// Reports each index of `place` not proven within the array it indexes.
    fn indices(&mut self, place: &Place, origin: OriginId) {
        let types = place.types(self.env.cx, &self.body.locals);
        for (projection, ty) in place.projection.iter().zip(&types) {
            let Projection::Index(index) = projection else { continue };
            let bounds = ty.and_then(|t| t.as_array().and_then(|(len, _)| len.to_i128()));
            let index = self.body.locals[*index].ty.and_then(|t| t.interval());
            let within = |len: i128| {
                Interval::new(Extended::Finite(Integer::from(0)), Extended::Finite(Integer::from(len - 1)))
            };
            let proven =
                bounds.zip(index).is_some_and(|(len, i)| within(len).is_some_and(|b| b.contains(&i)));
            if !proven {
                self.report(origin, Msg::new().text("this index is not proven within bounds, which traps"));
            }
        }
    }

    /// The integers `operand` may hold, when its type says.
    fn interval(&self, operand: &Operand) -> Option<Interval> {
        match operand {
            Operand::Const(Value::Int(n)) => Some(Interval::singleton(n.clone())),
            Operand::Place(p) if p.projection.is_empty() => self.body.locals[p.local].ty?.interval(),
            _ => None,
        }
    }

    fn report(&mut self, origin: OriginId, msg: Msg) {
        let d = Diagnostic::error(errors::PANIC_NEVER_VIOLATED, msg).at(origin, None);
        self.diag.error(d);
    }
}
