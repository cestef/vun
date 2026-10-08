//! Expressions as source.

use vun_items::lang;
use vun_syntax::make::{Arm as SourceArm, Block, Borrow, Expr as Source, Function, Init, Param};

use super::{Body, Lowering, names};
use vun_grammar::TokenKind;

use crate::float::Format;
use crate::ir::{Bound, Capturing, Convert, Expr, Kind, LocalId, Pattern, Place, Projection};
use crate::ty::Ty;
use crate::value::Value;

impl Body<'_> {
    /// The function as a function literal.
    pub(super) fn lowered(&self) -> Source {
        let function = self.function;
        let generics = function.generics.iter().enumerate().map(|(at, bound)| Param {
            pattern: Some(Source::prefix(TokenKind::Dollar, Source::local(&names::generic(at)))),
            ty: self.lowering.bound(bound),
        });
        let params = generics
            .chain(
                function
                    .locals
                    .iter()
                    .take(function.params)
                    .map(|(id, local)| Param::named(&self.name(id), self.lowering.ty(&local.ty))),
            )
            .collect();
        Source::Function(Function {
            params,
            result: Some(Box::new(self.lowering.result(function))),
            body: self.block(&function.body, Some(&function.tail)),
        })
    }

    pub(super) fn name(&self, id: LocalId) -> String {
        names::local(id, self.function.params)
    }

    pub(super) fn local(&self, id: LocalId) -> Source {
        Source::local(&self.name(id))
    }

    /// A value as a literal.
    fn value(&self, value: &Value) -> Source {
        match value {
            Value::Int(n) => Lowering::integer(n),
            Value::Bool(b) => {
                self.lowering.prelude(if *b { lang::bool::r#true } else { lang::bool::r#false })
            }
            Value::Float(x) => Source::float(x.value()),
            Value::Unit | Value::Ref(_) | Value::Closure(..) => Source::Unit,
            Value::Tuple(parts) => Source::Tuple(parts.iter().map(|p| self.value(p)).collect()),
            Value::Array(items) => Source::List(items.iter().map(|i| self.value(i)).collect()),
            Value::Variant(..) => unreachable!("the program has no literal of an enum"),
        }
    }

    /// The expression as source.
    #[allow(clippy::too_many_lines, reason = "one arm per form, as declared")]
    pub(super) fn expr(&self, expr: &Expr) -> Source {
        let lowering = self.lowering;
        match &expr.kind {
            Kind::Lit(Value::Float(x)) => self.float(*x, &expr.ty),
            Kind::Lit(value) => self.value(value),
            Kind::Local(id) => self.local(*id),
            Kind::Unary(op, operand) => Source::prefix(op.token(), self.expr(operand)),
            Kind::Arith(op, left, right) => Source::infix(op.token(), self.expr(left), self.expr(right)),
            Kind::Compare(op, left, right) => Source::infix(op.token(), self.expr(left), self.expr(right)),
            Kind::Logic(op, left, right) => Source::infix(op.token(), self.expr(left), self.expr(right)),
            Kind::Narrow(how, operand) => {
                Source::method(self.expr(operand), how.method().name(), vec![lowering.ty(&expr.ty)])
            }
            Kind::If(condition, then, otherwise) => Source::when(
                self.expr(condition),
                Block::of(self.expr(then)),
                Some(Source::Block(Block::of(self.expr(otherwise)))),
            ),
            Kind::Tuple(items) => Source::Tuple(items.iter().map(|i| self.expr(i)).collect()),
            Kind::Array(items) => Source::List(items.iter().map(|i| self.expr(i)).collect()),
            Kind::Repeat(item, count) => {
                Source::Repeat(Box::new(self.expr(item)), Box::new(Source::int(*count)))
            }
            Kind::Field(whole, at) => match &whole.ty {
                Ty::Struct(..) => Source::member(self.expr(whole), &names::field(*at)),
                _ => Source::field_at(self.expr(whole), u32::try_from(*at).expect("a tuple has few parts")),
            },
            Kind::Index(array, index) => Source::index(self.expr(array), self.expr(index)),
            Kind::Call(function, generics, args) => Source::call(
                Source::local(&names::function(*function)),
                generics
                    .iter()
                    .zip(&lowering.program.functions[*function].generics)
                    .filter(|(_, bound)| **bound == Bound::Any)
                    .map(|(g, _)| lowering.ty(g))
                    .chain(args.iter().map(|a| self.expr(a)))
                    .collect(),
            ),
            Kind::Block(stmts, tail) => Source::Block(self.block(stmts, Some(tail))),
            Kind::TailCall(function, args) => Source::call(
                Source::decl(lang::flow::tail),
                vec![
                    Source::local(&names::function(*function)),
                    Source::Tuple(args.iter().map(|a| self.expr(a)).collect()),
                ],
            ),
            Kind::Forever(label, stmts) => {
                Source::Labeled(names::label(*label), Box::new(Source::For(None, self.block(stmts, None))))
            }
            Kind::Closure(closure) => {
                let captures = closure
                    .captures
                    .iter()
                    .map(|capture| match capture.how {
                        Capturing::Copy => self.local(capture.local),
                        Capturing::Shared => Source::borrow(Borrow::SHARED, self.local(capture.local)),
                        Capturing::Mut => Source::borrow(Borrow::MUT, self.local(capture.local)),
                    })
                    .collect();
                let params = closure
                    .params
                    .iter()
                    .map(|&p| Param::named(&self.name(p), lowering.ty(&self.function.locals[p].ty)))
                    .collect();
                let body = match &closure.body.kind {
                    Kind::Block(stmts, tail) => self.block(stmts, Some(tail)),
                    _ => Block::of(self.expr(&closure.body)),
                };
                let result = closure.result.as_ref().map(|r| Box::new(lowering.ty(r)));
                Source::Closure(captures, Function { params, result, body })
            }
            Kind::CallClosure(callee, args) => {
                Source::call(self.expr(callee), args.iter().map(|a| self.expr(a)).collect())
            }
            Kind::Borrow(mutable, place) => {
                let mode = if *mutable { Borrow::MUT } else { Borrow::SHARED };
                Source::borrow(mode, self.place(place))
            }
            Kind::Deref(reference) => Source::postfix(TokenKind::DotStar, self.expr(reference)),
            Kind::Convert(how, operand) => {
                Source::method(self.expr(operand), how.method(), vec![lowering.ty(&expr.ty)])
            }
            Kind::TryTo(operand) => {
                let Ty::Enum(_, variants) = &expr.ty else { return Source::Hole };
                Source::method(self.expr(operand), "try_to", vec![lowering.ty(&variants[1][0])])
            }
            Kind::Try(operand) => Source::Postfix(TokenKind::Question, Box::new(self.expr(operand))),
            Kind::UnwrapOr(option, default) => {
                Source::method(self.expr(option), "unwrap_or", vec![self.expr(default)])
            }
            Kind::IsSome(option) => Source::method(self.expr(option), "is_some", Vec::new()),
            Kind::Struct(id, fields) => {
                let inits = fields
                    .iter()
                    .enumerate()
                    .map(|(at, f)| Init::Named(names::field(at), self.expr(f)))
                    .collect();
                Source::Typed(Box::new(Source::local(&names::ty(*id, true))), inits)
            }
            Kind::Variant(nominal, variant, payload) => {
                let constructor =
                    Source::member(lowering.nominal(*nominal, &expr.ty), &names::variant(*nominal, *variant));
                if payload.is_empty() {
                    constructor
                } else {
                    Source::call(constructor, payload.iter().map(|p| self.expr(p)).collect())
                }
            }
            Kind::Match(scrutinee, arms) => {
                let arms = arms
                    .iter()
                    .map(|arm| SourceArm {
                        pattern: self.pattern(&arm.pattern, &scrutinee.ty),
                        guard: None,
                        value: Some(self.expr(&arm.body)),
                    })
                    .collect();
                Source::matching(self.expr(scrutinee), arms)
            }
        }
    }

    /// A float literal, of the type it is drawn from: converted from the `f64` literal that has
    /// the same value, where its type is another.
    fn float(&self, x: crate::float::Float, ty: &Ty) -> Source {
        let value = x.value();
        let magnitude = Source::float(value.abs());
        let literal =
            if value.is_sign_negative() { Source::prefix(TokenKind::Minus, magnitude) } else { magnitude };
        match ty {
            Ty::Float(Format::F64) => literal,
            // No backend converts to `f16` from anything but `f32`.
            Ty::Float(Format::F16) => {
                let single = Source::method(
                    literal,
                    Convert::Nearest.method(),
                    vec![self.lowering.ty(&Ty::Float(Format::F32))],
                );
                Source::method(single, Convert::Nearest.method(), vec![self.lowering.ty(ty)])
            }
            _ => Source::method(literal, Convert::Nearest.method(), vec![self.lowering.ty(ty)]),
        }
    }

    /// The place as an expression: the local, what it refers to, the parts reached.
    pub(super) fn place(&self, place: &Place) -> Source {
        let local = self.local(place.local);
        let base = if place.deref { Source::postfix(TokenKind::DotStar, local) } else { local };
        place.steps.iter().fold(base, |at, step| match step {
            Projection::Field(i) => Source::field_at(at, u32::try_from(*i).expect("a tuple has few parts")),
            Projection::Member(i) => Source::member(at, &names::field(*i)),
            Projection::Index(index) => Source::index(at, self.expr(index)),
        })
    }

    fn pattern(&self, pattern: &Pattern, scrutinee: &Ty) -> Source {
        match pattern {
            Pattern::Any => Source::Hole,
            Pattern::Variant { variant, binds } => {
                let binds = binds.iter().map(|b| b.map_or(Source::Hole, |local| self.local(local))).collect();
                let Ty::Enum(nominal, _) = scrutinee else { return Source::Hole };
                Source::variant(&names::variant(*nominal, *variant), binds)
            }
        }
    }
}
