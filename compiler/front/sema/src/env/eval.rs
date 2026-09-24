//! Evaluating constant forms, and the types annotations denote.

use overt_grammar::{BORROWS, TYPE_PREFIXES, TokenKind};
use overt_hir::resolve::Res;
use overt_hir::tree::{Arg, BlockId, Cond, Entry, ExprId, ExprKind, List, Lit, Name, Param};
use overt_intern::Symbol;
use overt_origin::OriginTable;

use super::{Env, Unevaluated};
use crate::eval::{Call, Instance};
use crate::int::Integer;
use crate::item::ExprRef;
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
            ExprKind::Name(_) => match self.parsed.resolution.names[expr] {
                Some(Res::Binder(b)) if let Some(bound) = self.bindings.get(&b) => Ok(bound.clone()),
                _ => self.value_of(self.resolve(expr)?),
            },
            ExprKind::Member { object, name: Name::Plain(name) } => self.member(self.eval(*object)?, *name),
            ExprKind::Enum(_) => Ok(self.nominal(expr, NominalKind::Enum)),
            ExprKind::Struct(_) => Ok(self.nominal(expr, NominalKind::Struct)),
            ExprKind::Prefix { op: TokenKind::Amp, mode, operand } => self.borrow(*mode, *operand),
            ExprKind::Prefix { op, operand, .. }
                if let Some((_, item)) = TYPE_PREFIXES.iter().find(|(t, _)| t == op) =>
            {
                let ty = self.ty(*operand)?;
                self.apply(self.item(item)?, vec![Value::Type(ty)])
            }
            ExprKind::ArrayType { len, elem } => {
                let Value::Int(len) = self.eval(*len)? else { return Err(Unevaluated::Unsupported) };
                Ok(Value::Type(TyKind::Array { len, elem: self.ty(*elem)? }.intern()))
            }
            ExprKind::Array(items) => items
                .iter()
                .map(|item| match item {
                    Entry::One(item) => self.eval(*item),
                    Entry::Staged(_) => Err(Unevaluated::Unsupported),
                })
                .collect::<Result<_, _>>()
                .map(Value::Array),
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
                    self.block(*then)
                } else {
                    otherwise.map_or_else(|| Ok(Value::Tuple(Vec::new())), |e| self.eval(e))
                }
            }
            ExprKind::Block(block) => self.block(*block),
            ExprKind::Call { callee, args } => self.call(*callee, args).map_err(|u| u.through(e.origin)),
            ExprKind::Fn { .. } => Ok(Value::Fn(ExprRef::new(self.file.clone(), hir, expr))),
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
    fn block(&self, block: BlockId) -> Result<Value, Unevaluated> {
        let block = &self.parsed.hir.blocks[block];
        if !block.stmts.is_empty() {
            return Err(Unevaluated::Interpreter);
        }
        block.tail.map_or_else(|| Ok(Value::Tuple(Vec::new())), |t| self.eval(t))
    }

    /// A call of a function value, which the evaluator runs, or of a primitive.
    fn call(&self, callee: ExprId, args: &[Entry<Arg>]) -> Result<Value, Unevaluated> {
        let callee = self.eval(callee)?;
        let args = args
            .iter()
            .map(|arg| match arg {
                Entry::One(Arg { name: None, value }) => self.eval(*value),
                _ => Err(Unevaluated::Unsupported),
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.apply(callee, args)
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
            Value::Primitive(p) => p.apply(&args),
            _ => Err(Unevaluated::Unsupported),
        }
    }

    /// `&T`, `&mut T`, ...: the borrow type the word after `&` names (appendix B), applied to the
    /// type `operand`. A borrow of a value exists at run time only.
    fn borrow(&self, mode: Option<Symbol>, operand: ExprId) -> Result<Value, Unevaluated> {
        let word = mode.map_or("", Symbol::as_str);
        let (_, item) = BORROWS.iter().find(|(m, _)| *m == word).ok_or(Unevaluated::Unsupported)?;
        let ty = self.eval(operand)?.as_type().ok_or(Unevaluated::Runtime)?;
        self.apply(self.item(item)?, vec![Value::Type(ty)])
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
                Entry::One(Param { ty: Some(t), .. }) => self.ty(*t),
                _ => Err(Unevaluated::Unsupported),
            })
            .collect::<Result<_, _>>()?;
        Ok(Signature { params, result: self.result(returns)? })
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
