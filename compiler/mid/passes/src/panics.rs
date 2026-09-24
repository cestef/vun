//! `#[panic.never]` (spec 07, section 3): a function promising never to trap contains no trap
//! site its types do not discharge, and calls only functions making the same promise. Trap sites
//! here: a division by a value that may be 0, a narrowing that may not fit (`Narrowing::To`), and
//! an index not proven within an array; a call of a function without the promise may trap too.

use vun_diag::{DiagCtxt, Diagnostic, Msg, errors};
use vun_grammar::{Narrowing, TokenKind};
use vun_mir::{Body, Mir, Operand, Place, Projection, Rvalue};
use vun_origin::OriginId;
use vun_query::{Cx, Query};
use vun_sema::env::Env;
use vun_sema::eval::Instantiation;
use vun_sema::int::{Extended, Integer, Interval};
use vun_sema::ty::{Marker, Value};

use crate::types::PlaceTypes;

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
            Rvalue::Call { callee, .. } if self.may_trap(callee) => {
                self.report(
                    origin,
                    Msg::new().text("this call may trap: the function called is not ").code("#[panic.never]"),
                );
            }
            Rvalue::Binary { op: TokenKind::Slash | TokenKind::Percent, right, .. }
                if self
                    .interval(right)
                    .is_none_or(|i| i.contains(&Interval::singleton(Integer::from(0)))) =>
            {
                self.report(origin, Msg::new().text("this divides by a value that may be 0"));
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
            | Rvalue::Resume { frame: place, .. } => {
                self.indices(place, origin);
            }
            _ => {}
        }
    }

    /// Whether calling `callee` may trap: a function or primitive without the promise; a
    /// variant's constructor never does, and a callee of unknown type says nothing.
    fn may_trap(&self, callee: &Operand) -> bool {
        let markers = match callee {
            Operand::Const(Value::Fn(f) | Value::Closure { f, .. }) => {
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
        let types = PlaceTypes { cx: self.env.cx, body: self.body }.along(place);
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
