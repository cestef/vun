//! Evaluating constant forms, and the types annotations denote.

use std::sync::Arc;

use vun_desugar::{Access, Borrow, Form, MARKERS, TYPE_PREFIXES};
use vun_grammar::TokenKind;
use vun_hir::tree::{BlockId, Cond, Entry, ExprId, ExprKind, List, Lit, Param, PatId, PatKind};
use vun_origin::OriginTable;
use vun_sema_float::{Binary, Format};
use vun_sema_int::Integer;
use vun_sema_ty::call::Func;
use vun_sema_ty::float::FloatType;
use vun_sema_ty::item::ExprRef;
use vun_sema_ty::ops::Part;
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Nominal, NominalKind, Signature, Ty, TyKind, Value};

use super::Env;

impl Env<'_, '_> {
    /// The value of `expr` at compile time.
    ///
    /// # Errors
    /// Returns why it has no value at compile time here.
    pub fn eval(&self, expr: ExprId) -> Result<Value, Unevaluated> {
        let hir = &self.parsed.hir;
        let e = &hir.exprs[expr];
        match &e.kind {
            ExprKind::Lit(lit) => self.literal(expr, lit),
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
            ExprKind::Borrow { mode, operand } => {
                self.borrow(expr, mode.ok_or(Unevaluated::Unsupported)?, *operand)
            }
            ExprKind::Prefix { op, operand, .. }
                if let Some(row) = TYPE_PREFIXES.iter().find(|row| row.token == *op) =>
            {
                let ty = self.ty(*operand)?;
                self.apply(self.item(row.item)?, vec![Value::Type(ty)])
            }
            ExprKind::Prefix { op, operand, .. } if MARKERS.iter().any(|row| row.token == *op) => {
                self.marked(*op, *operand)
            }
            ExprKind::ArrayType { len, elem } => {
                let (len, elem) = (self.eval(*len)?, self.ty(*elem)?);
                self.apply(self.form(Form::Array)?, vec![len, Value::Type(elem)])
            }
            ExprKind::Array(items) => {
                let item = self.type_of(expr).and_then(|t| t.items());
                self.items(items)?
                    .iter()
                    .map(|(env, e)| env.eval_as(**e, item))
                    .collect::<Result<_, _>>()
                    .map(Value::Array)
            }
            ExprKind::Repeat { value, count } => {
                let item = self.eval_as(*value, self.type_of(expr).and_then(|t| t.items()))?;
                Value::repeat(item, &self.eval(*count)?)
            }
            ExprKind::Prefix { op: TokenKind::Dollar, operand, .. } => self.splice(self.eval(*operand)?),
            ExprKind::Postfix { op: TokenKind::DotStar, operand } => self.reached(*operand),
            ExprKind::Match { scrutinee, arms } => self.matching(*scrutinee, arms),
            // `!c` is the other condition; `-a` and `~a` are operators.
            ExprKind::Prefix { op: TokenKind::Bang, operand, .. } => {
                self.decide(!self.holds(&self.eval(*operand)?)?)
            }
            ExprKind::Prefix { op, operand, .. } => self.prefix(*op, self.operand(*operand)?),
            ExprKind::Infix { op: op @ (TokenKind::DotDot | TokenKind::DotDotEq), left, right } => {
                let hi = right.map(|r| self.eval(r)).transpose()?;
                Value::range(*op, &self.eval(*left)?, hi.as_ref())
            }
            // `and` and `or` evaluate their right operand only when it decides.
            ExprKind::Infix { op: op @ (TokenKind::And | TokenKind::Or), left, right: Some(right) } => {
                let left = self.eval(*left)?;
                if self.holds(&left)? == (*op == TokenKind::Or) { Ok(left) } else { self.eval(*right) }
            }
            ExprKind::Infix { op, left, right: Some(right) } => self.infix(*op, *left, *right),
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
            ExprKind::Call { callee, args } => self.call(expr, *callee, args),
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
            ExprKind::Tuple(items) => self
                .items(items)?
                .iter()
                .map(|(env, e)| env.eval(**e))
                .collect::<Result<_, _>>()
                .map(Value::Tuple),
            _ => Err(Unevaluated::Unsupported),
        }
    }

    /// The items of a tuple or an array literal, generated ones included, each with the
    /// environment it is evaluated in.
    fn items<'e>(&self, items: &'e List<ExprId>) -> Result<Vec<(Self, &'e ExprId)>, Unevaluated> {
        let mut out = Vec::new();
        self.list(items, &mut out)?;
        Ok(out)
    }

    /// The value of a block without statements: its tail, or `()`. Statements run only inside
    /// a call, in the evaluator.
    pub(super) fn block_value(&self, block: BlockId) -> Result<Value, Unevaluated> {
        let block = &self.parsed.hir.blocks[block];
        if !block.stmts.is_empty() {
            return Err(Unevaluated::Unsupported);
        }
        block.tail.map_or_else(|| Ok(Value::Tuple(Vec::new())), |t| self.eval(t))
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

    /// `&T`, `&mut T`, ...: the borrow type of the mode (appendix B), applied to the type
    /// `operand`. Any other borrow of a value needs its place, which only the evaluator holds,
    /// inside a call.
    fn borrow(&self, expr: ExprId, mode: Borrow, operand: ExprId) -> Result<Value, Unevaluated> {
        // Typed as a borrow, `&t` borrows the value `t`, even when that value is a type.
        let pointee = self.type_of(expr).and_then(|t| t.pointee());
        let value = self.eval_as(operand, pointee)?;
        match value.as_type() {
            Some(ty) if pointee.is_none() => self.apply(self.item(mode.item())?, vec![Value::Type(ty)]),
            // A shared borrow of a compile-time value reads the value, which never changes.
            _ if mode.is_plain() => Ok(value),
            _ => Err(Unevaluated::Unsupported),
        }
    }

    /// `operand.*`: through a shared borrow, the value it reads. Other pointers are followed by
    /// the evaluator, which holds the places they reach.
    fn reached(&self, operand: ExprId) -> Result<Value, Unevaluated> {
        match self.type_of(operand).and_then(|t| t.borrowing()) {
            Some((mode, ..)) if mode.access() == Access::Shared => self.eval(operand),
            _ => Err(Unevaluated::Unsupported),
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
                Entry::One(Param { pat, ty: Some(t), .. }) => match self.eval(*t) {
                    Ok(Value::Interface(_)) => Ok(TyKind::of_types()),
                    _ => self.param_type(*pat, self.ty(*t)?),
                },
                _ => Err(Unevaluated::Unsupported),
            })
            .collect::<Result<_, _>>()?;
        Ok(Signature::new(params, self.result(returns)?))
    }

    /// The type a function's signature gives the parameter `pat` annotated `ty`: a pack's,
    /// `lang.Pack(ty)`, for the rest pattern `..args` gathering the arguments after the others.
    ///
    /// # Errors
    /// Returns why `lang.Pack(ty)` has no value: a type that is not a tuple.
    pub fn param_type(&self, pat: Option<PatId>, ty: Ty) -> Result<Ty, Unevaluated> {
        match pat.map(|p| &self.parsed.hir.pats[p].kind) {
            Some(PatKind::Rest(_)) => {
                let pack = self.item(vun_items::lang::Pack)?;
                self.apply(pack, vec![Value::Type(ty)])?.as_type().ok_or(Unevaluated::Unsupported)
            }
            _ => Ok(ty),
        }
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
        let name = OriginTable::global().declaration_name(self.parsed.hir.exprs[literal].origin);
        let literal = ExprRef::new(self.file.clone(), &self.parsed.hir, literal);
        Value::Type(
            TyKind::Nominal(Nominal { literal, instance: self.instance.clone(), kind, name }).intern(),
        )
    }

    /// The literal `lit` at `expr`; a float rounded to the float type typing gave it.
    fn literal(&self, expr: ExprId, lit: &Lit) -> Result<Value, Unevaluated> {
        match lit {
            Lit::Int(n) => Ok(Value::Int(Integer::from(n.clone()))),
            Lit::Char(c) => Ok(Value::Int(Integer::from(i128::from(u32::from(*c))))),
            Lit::String(bytes) => Ok(Value::bytes(bytes)),
            Lit::Float(text) => {
                let format = self.type_of(expr).and_then(|t| Format::of(&t)).unwrap_or(Format::DEFAULT);
                Binary::parse(text, format).map(Value::Float).ok_or(Unevaluated::Unsupported)
            }
        }
    }
}
