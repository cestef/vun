//! Evaluating constant forms, and the types annotations denote.

use std::sync::Arc;

use vun_desugar::{BORROWS, Form, MARKERS, TYPE_PREFIXES};
use vun_grammar::TokenKind;
use vun_hir::tree::{Arg, BlockId, Cond, Entry, ExprId, ExprKind, List, Lit, Name, Param};
use vun_intern::Symbol;
use vun_origin::OriginTable;

use super::{Env, Unevaluated};
use crate::eval::{Call, Func, Instance};
use crate::int::Integer;
use crate::item::ExprRef;
use crate::ops::Part;
use crate::ty::{Nominal, NominalKind, Signature, Ty, TyKind, Value};

impl Env<'_, '_> {
    /// The value of `expr` at compile time.
    ///
    /// # Errors
    /// Returns why it has no value at compile time here.
    pub fn eval(&self, expr: ExprId) -> Result<Value, Unevaluated> {
        let hir = &self.parsed.hir;
        let e = &hir.exprs[expr];
        match &e.kind {
            ExprKind::Lit(Lit::Int(n)) => Ok(Value::Int(Integer::from(*n))),
            ExprKind::Lit(Lit::Char(c)) => Ok(Value::Int(Integer::from(i128::from(u32::from(*c))))),
            ExprKind::Lit(Lit::String(text)) => Ok(Value::string(text)),
            ExprKind::StructLit { ty: Some(ty), fields } => self.record(self.ty(*ty)?, fields),
            ExprKind::StructLit { ty: None, .. } | ExprKind::DotName(_) => self.eval_as(expr, None),
            ExprKind::Index { object, index } => self.index(*object, *index),
            ExprKind::Name(_) => self.named(expr),
            ExprKind::Member { object, name } => match (self.name(name), self.position(name)) {
                (Some(name), _) => self.member(self.eval(*object)?, name),
                (None, Some(at)) => self.eval(*object)?.part(&Part::Position(at)),
                (None, None) => Err(Unevaluated::Unsupported),
            },
            ExprKind::Enum { variants, .. } if variants.is_empty() => {
                Ok(self.nominal(expr, NominalKind::Empty))
            }
            ExprKind::Enum { .. } => Ok(self.nominal(expr, NominalKind::Enum)),
            ExprKind::Struct(_) => Ok(self.nominal(expr, NominalKind::Struct)),
            ExprKind::Prefix { op: TokenKind::Amp, mode, operand } => self.borrow(expr, *mode, *operand),
            ExprKind::Prefix { op, operand, .. }
                if let Some((_, item)) = TYPE_PREFIXES.iter().find(|(t, _)| t == op) =>
            {
                let ty = self.ty(*operand)?;
                self.apply(self.item(*item)?, vec![Value::Type(ty)])
            }
            ExprKind::Prefix { op, operand, .. } if MARKERS.iter().any(|(t, _)| t == op) => {
                self.marked(*op, *operand)
            }
            ExprKind::ArrayType { len, elem } => {
                let (len, elem) = (self.eval(*len)?, self.ty(*elem)?);
                self.apply(self.form(Form::Array)?, vec![len, Value::Type(elem)])
            }
            ExprKind::Array(items) => items
                .iter()
                .map(|item| match item {
                    Entry::One(item) => self.eval_as(*item, self.type_of(expr).and_then(|t| t.items())),
                    Entry::Staged(_) => Err(Unevaluated::Unsupported),
                })
                .collect::<Result<_, _>>()
                .map(Value::Array),
            ExprKind::Repeat { value, count } => {
                let item = self.eval_as(*value, self.type_of(expr).and_then(|t| t.items()))?;
                Value::repeat(item, &self.eval(*count)?)
            }
            ExprKind::Prefix { op: TokenKind::Dollar, operand, .. } => self.splice(self.eval(*operand)?),
            ExprKind::Prefix { op, operand, .. } => self.unary(*op, &self.eval(*operand)?),
            ExprKind::Infix { op: op @ (TokenKind::DotDot | TokenKind::DotDotEq), left, right } => {
                let hi = right.map(|r| self.eval(r)).transpose()?;
                Value::range(*op, &self.eval(*left)?, hi.as_ref())
            }
            // `and` and `or` evaluate their right operand only when it decides.
            ExprKind::Infix { op: op @ (TokenKind::And | TokenKind::Or), left, right: Some(right) } => {
                let left = self.eval(*left)?;
                if self.holds(&left)? == (*op == TokenKind::Or) { Ok(left) } else { self.eval(*right) }
            }
            ExprKind::Infix { op, left, right: Some(right) } => {
                self.binary(*op, &self.eval(*left)?, &self.eval(*right)?)
            }
            ExprKind::If { cond: Cond::Expr(cond), then, otherwise } => {
                if self.holds(&self.eval(*cond)?)? {
                    self.block_value(*then)
                } else {
                    otherwise.map_or_else(|| Ok(Value::Tuple(Vec::new())), |e| self.eval(e))
                }
            }
            ExprKind::Block(block) => self.block_value(*block),
            ExprKind::Quote(_) => Ok(Value::Code(self.code(expr))),
            // Attributes on an expression are data, read where they act.
            ExprKind::Attributed { attrs, inner } => Ok(self.marked_by(self.eval(*inner)?, attrs)),
            ExprKind::Call { callee, .. } if matches!(hir.exprs[*callee].kind, ExprKind::DotName(_)) => {
                self.eval_as(expr, None)
            }
            ExprKind::Call { callee, args } => self.call(*callee, args).map_err(|u| u.through(e.origin)),
            ExprKind::Fn { .. } => Ok(Value::Fn(self.func(expr))),
            ExprKind::Closure { captures, func } => {
                let captures = captures
                    .iter()
                    .map(|c| match c {
                        Entry::One(c) => self.eval(*c),
                        Entry::Staged(_) => Err(Unevaluated::Unsupported),
                    })
                    .collect::<Result<_, _>>()?;
                Ok(Value::Closure { f: self.func(*func), captures })
            }
            ExprKind::FnType { params, returns } => Ok(Value::Type(self.signature(params, *returns)?.ty())),
            ExprKind::Tuple(items) => items
                .iter()
                .map(|item| match item {
                    Entry::One(item) => self.eval(*item),
                    Entry::Staged(_) => Err(Unevaluated::Unsupported),
                })
                .collect::<Result<_, _>>()
                .map(Value::Tuple),
            _ => Err(Unevaluated::Unsupported),
        }
    }

    /// The value of a block without statements: its tail, or `()`. Statements run in the
    /// evaluator.
    pub(super) fn block_value(&self, block: BlockId) -> Result<Value, Unevaluated> {
        let block = &self.parsed.hir.blocks[block];
        if !block.stmts.is_empty() {
            return Err(Unevaluated::Interpreter);
        }
        block.tail.map_or_else(|| Ok(Value::Tuple(Vec::new())), |t| self.eval(t))
    }

    /// A call of a function value, which the evaluator runs, or of a primitive. `a.f(b)` on a
    /// value `a` of type `T` is `T.f(a, b)` (spec 01, section 4).
    fn call(&self, callee: ExprId, args: &[Entry<Arg>]) -> Result<Value, Unevaluated> {
        let mut values = Vec::new();
        let callee = match &self.parsed.hir.exprs[callee].kind {
            ExprKind::Member { object, name: Name::Plain(name) } => {
                let ty = self.type_of(*object);
                let object = self.eval(*object)?;
                // A member of a namespace or of a type is a declaration, never a method of the
                // namespace or of `type`.
                let declared = matches!(object, Value::Namespace(_) | Value::Type(_));
                let ty = if declared { None } else { object.ty().or(ty) };
                match ty {
                    // Through borrows, as `.` goes: a compile-time borrow is its value.
                    Some(ty) => {
                        values.push(object);
                        let mut types = std::iter::successors(Some(ty), |t| t.borrowed());
                        types
                            .find_map(|t| self.member(Value::Type(t), *name).ok())
                            .ok_or(Unevaluated::Unsupported)?
                    }
                    None => self.member(object, *name)?,
                }
            }
            _ => self.eval(callee)?,
        };
        self.call_value(callee, values, args)
    }

    /// `callee(values..., args)`: the arguments evaluated after `values`, which a receiver fills.
    pub(super) fn call_value(
        &self,
        callee: Value,
        mut values: Vec<Value>,
        args: &[Entry<Arg>],
    ) -> Result<Value, Unevaluated> {
        // A `Code(T)` parameter receives its argument as code; an argument typed by its context,
        // such as `.{ ... }`, is evaluated as its parameter's type where that type is known.
        let (quotes, params) = match &callee {
            Value::Fn(f) => {
                let signature = Env::inside(self.cx, f, [])
                    .and_then(|env| env.literal_signature(f.literal.expr.id, &[]).ok());
                (f.quotes(self.cx).unwrap_or_default(), signature.map(|s| s.params).unwrap_or_default())
            }
            _ => (Vec::new(), Vec::new()),
        };
        for arg in args {
            match arg {
                Entry::One(Arg { name: None, value }) if quotes.get(values.len()) == Some(&true) => {
                    values.push(Value::Code(self.code(*value)));
                }
                Entry::One(Arg { name: None, value }) => {
                    values.push(self.eval_as(*value, params.get(values.len()).copied())?);
                }
                _ => return Err(Unevaluated::Unsupported),
            }
        }
        self.apply(callee, values)
    }

    /// `callee(args)` at compile time: a function value, which the evaluator runs, or a
    /// primitive.
    ///
    /// # Errors
    /// Returns why the call has no value.
    pub fn apply(&self, callee: Value, args: Vec<Value>) -> Result<Value, Unevaluated> {
        match callee {
            // An interface applied to a type is its requirement struct (spec 02, section 3).
            Value::Fn(f) | Value::Interface(f) => self.cx.get::<Call>(&Instance { f, args }),
            Value::Closure { f, captures } => {
                self.cx.get::<Call>(&Instance { f, args: captures.into_iter().chain(args).collect() })
            }
            Value::Primitive(p) => p.apply(self, &args),
            Value::Constructor { ty, name } => Ok(Value::Variant { ty, name, payload: args }),
            // A link symbol runs at run time only.
            Value::Linked(_) => Err(Unevaluated::Runtime),
            _ => Err(Unevaluated::Unsupported),
        }
    }

    /// `unsafe (A) -> R`: the prefix `op` marks a function type (the table `markers`); any other
    /// operand keeps its value.
    fn marked(&self, op: TokenKind, operand: ExprId) -> Result<Value, Unevaluated> {
        let value = self.eval(operand)?;
        match value.as_type().and_then(|t| t.signature()) {
            Some(signature) => Ok(Value::Type(signature.marked(op).ty())),
            None => Ok(value),
        }
    }

    /// `&T`, `&mut T`, ...: the borrow type the word after `&` names (appendix B), applied to the
    /// type `operand`. A borrow of a value exists at run time only.
    fn borrow(&self, expr: ExprId, mode: Option<Symbol>, operand: ExprId) -> Result<Value, Unevaluated> {
        let word = mode.map_or("", Symbol::as_str);
        let (_, item, ..) = BORROWS.iter().find(|(m, ..)| *m == word).ok_or(Unevaluated::Unsupported)?;
        let value = self.eval_as(operand, self.type_of(expr).and_then(|t| t.pointee()))?;
        match value.as_type() {
            Some(ty) => self.apply(self.item(*item)?, vec![Value::Type(ty)]),
            // A shared borrow of a compile-time value reads the value, which never changes.
            None if word.is_empty() => Ok(value),
            None => Err(Unevaluated::Runtime),
        }
    }

    /// The type `expr` denotes, evaluated where a type is expected.
    ///
    /// # Errors
    /// Returns why it has no value here, or [`Unevaluated::Unsupported`] for a value that is not
    /// a type.
    pub fn ty(&self, expr: ExprId) -> Result<Ty, Unevaluated> {
        self.eval(expr)?.as_type().ok_or(Unevaluated::Unsupported)
    }

    /// The result type a function declares: `()` when it declares none.
    ///
    /// # Errors
    /// As [`Self::ty`].
    pub fn result(&self, returns: Option<ExprId>) -> Result<Ty, Unevaluated> {
        returns.map_or_else(|| Ok(TyKind::unit()), |r| self.ty(r))
    }

    /// The signature `params -> returns` declares.
    ///
    /// # Errors
    /// Returns why a parameter or result type has no value here, such as a type naming a `$`
    /// parameter, known only per instantiation.
    pub fn signature(&self, params: &List<Param>, returns: Option<ExprId>) -> Result<Signature, Unevaluated> {
        let params = params
            .iter()
            .map(|param| match param {
                // `$W: Writer`: a parameter constrained by an interface holds a type.
                Entry::One(Param { ty: Some(t), .. }) => match self.eval(*t) {
                    Ok(Value::Interface(_)) => Ok(TyKind::of_types()),
                    _ => self.ty(*t),
                },
                _ => Err(Unevaluated::Unsupported),
            })
            .collect::<Result<_, _>>()?;
        Ok(Signature::new(params, self.result(returns)?))
    }

    /// The function value the function literal `literal` evaluates to here, inside the call
    /// evaluated in, if any.
    #[must_use]
    pub fn func(&self, literal: ExprId) -> Func {
        let within = self.instance.clone().map(Arc::new);
        Func {
            literal: ExprRef::new(self.file.clone(), &self.parsed.hir, literal),
            within,
            staged: Arc::from(self.staged.as_slice()),
        }
    }

    /// The type the `struct` or `enum` literal `literal` evaluates to here, identified by the
    /// call evaluated in, if any (decision 28).
    #[must_use]
    pub fn nominal(&self, literal: ExprId, kind: NominalKind) -> Value {
        let name =
            self.cx.db().resource::<OriginTable>().declaration_name(self.parsed.hir.exprs[literal].origin);
        let literal = ExprRef::new(self.file.clone(), &self.parsed.hir, literal);
        Value::Type(
            TyKind::Nominal(Nominal { literal, instance: self.instance.clone(), kind, name }).intern(),
        )
    }
}
