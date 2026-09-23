//! The type of each form typed so far. Any other form has its parts typed and no type itself.

use overt_grammar::TokenKind;
use overt_hir::resolve::Res;
use overt_hir::tree::{Arg, Entry, ExprId, ExprKind, Hir, Lit, Name};
use overt_hir::visit::{self, Visitor};
use overt_intern::Symbol;

use super::{Known, Stage, Typer};
use crate::Primitive;
use crate::decl::DeclType;
use crate::int::{Integer, Interval};
use crate::item::{Declared, Site};
use crate::primitives::lang::Lang;
use crate::ty::{TyKind, Value};

impl Typer<'_, '_, '_> {
    pub(super) fn infer(&mut self, hir: &Hir, expr: ExprId) -> Option<Known> {
        let compile = |ty| Some(Known { ty, stage: Stage::Compile });
        match &hir.exprs[expr].kind {
            ExprKind::Lit(Lit::Int(n)) => compile(TyKind::int(&Interval::singleton(Integer::from(*n)))),
            ExprKind::Lit(Lit::Char(c)) => {
                compile(TyKind::int(&Interval::singleton(Integer::from(i128::from(u32::from(*c))))))
            }
            ExprKind::Name(_) => self.name(expr),
            ExprKind::Member { object, name: Name::Plain(name) } => self.member(*object, *name),
            ExprKind::Block(block) => self.block_value(*block),
            ExprKind::Prefix { op: TokenKind::Dollar, operand, .. } => {
                let outer = std::mem::replace(&mut self.stage, Stage::Compile);
                let known = self.typed(*operand);
                self.stage = outer;
                known
            }
            ExprKind::Prefix { op: TokenKind::Minus, operand, .. } => {
                let known = self.typed(*operand)?;
                Some(Known { ty: TyKind::int(&known.ty.interval()?.neg()), ..known })
            }
            ExprKind::Infix { op, left, right: Some(right) } => {
                let (left, right) = (self.typed(*left), self.typed(*right));
                self.arithmetic(expr, *op, left?, right?)
            }
            ExprKind::Call { callee, args } => self.call(*callee, args),
            // A function literal is a value; its body is typed on its own.
            ExprKind::Fn { params, returns, .. } => compile(self.env.signature(params, *returns).ok()?.ty()),
            ExprKind::FnType { .. } | ExprKind::Struct(_) | ExprKind::Enum(_) => {
                compile(TyKind::primitive(Primitive::Lang(Lang::r#type)))
            }
            // Quoted code is typed where it is spliced.
            ExprKind::Quote(_) => None,
            _ => {
                visit::walk_expr(self, hir, expr);
                None
            }
        }
    }

    /// A local by the type it was bound with; any other name by its declaration's type.
    fn name(&mut self, expr: ExprId) -> Option<Known> {
        if let Some(Res::Binder(b)) = self.env.parsed.resolution.names[expr]
            && let Some(known) = self.locals.get(&b)
        {
            return Some(*known);
        }
        self.site(self.env.resolve(expr).ok()?)
    }

    /// A member of a namespace; members of other values are not typed yet.
    fn member(&mut self, object: ExprId, name: Symbol) -> Option<Known> {
        self.typed(object);
        // Only a path names a namespace, so nothing else is evaluated here.
        if !matches!(self.hir().exprs[object].kind, ExprKind::Name(_) | ExprKind::Member { .. }) {
            return None;
        }
        let Ok(Value::Namespace(ns)) = self.env.eval(object) else { return None };
        self.site(self.env.cx.get::<Declared>(&(ns, name))?)
    }

    fn site(&self, site: Site) -> Option<Known> {
        match site {
            Site::Decl(decl) => self.env.cx.get::<DeclType>(&decl).ok(),
            Site::Namespace(_) => None,
        }
    }

    /// An integer operation, by interval arithmetic (spec 04, rule 1). A run-time intermediate
    /// past the width cap is reported (rule 5) and has no type, so nothing depending on it is.
    fn arithmetic(&mut self, expr: ExprId, op: TokenKind, left: Known, right: Known) -> Option<Known> {
        let range = left.ty.interval()?.binary(op, &right.ty.interval()?)?;
        let stage = left.stage.max(right.stage);
        if stage == Stage::Run && range.storage().is_none() && range.bits().is_some() {
            self.width_cap(expr, &range);
            return None;
        }
        Some(Known { ty: TyKind::int(&range), stage })
    }

    /// A call of a value of function type: its arguments are checked against the parameters, and
    /// it has the result type.
    fn call(&mut self, callee: ExprId, args: &[Entry<Arg>]) -> Option<Known> {
        let signature = self.typed(callee).and_then(|k| k.ty.signature());
        let hir = self.hir();
        let (mut found, mut positional) = (Vec::new(), true);
        for arg in args {
            match arg {
                Entry::One(Arg { name: None, value }) => found.push((*value, self.typed(*value))),
                Entry::One(Arg { value, .. }) => {
                    positional = false;
                    self.typed(*value);
                }
                Entry::Staged(staged) => {
                    positional = false;
                    self.staged(hir, staged, &mut |v, a: &Arg| {
                        v.typed(a.value);
                    });
                }
            }
        }
        let signature = signature.filter(|_| positional)?;
        if signature.params.len() != found.len() {
            return None;
        }
        for (&param, (value, known)) in signature.params.iter().zip(found) {
            if let Some(known) = known {
                self.expect(self.hir().exprs[value].origin, known, param, None);
            }
        }
        Some(Known { ty: signature.result, stage: self.stage })
    }
}
