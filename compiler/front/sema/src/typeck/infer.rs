//! The type of each form typed so far. Any other form has its parts typed and no type itself.

use overt_grammar::{OPERATORS, TokenKind};
use overt_hir::resolve::Res;
use overt_hir::tree::{Arg, Entry, ExprId, ExprKind, Hir, Lit, Name};
use overt_hir::visit;
use overt_intern::Symbol;

use super::{Known, Stage, Typer};
use crate::Primitive;
use crate::decl::DeclType;
use crate::env::Env;
use crate::eval::{Call, Instance};
use crate::int::{Integer, Interval};
use crate::item::{Declared, Site};
use crate::member::{Found, MemberType, Members};
use crate::primitives::lang::Lang;
use crate::ty::{Ty, TyKind, Value};

impl Typer<'_, '_, '_> {
    pub(super) fn infer(&mut self, hir: &Hir, expr: ExprId) -> Option<Known> {
        let compile = |ty| Some(Known { ty, stage: Stage::Compile });
        match &hir.exprs[expr].kind {
            ExprKind::Lit(Lit::Int(n)) => compile(TyKind::int(&Interval::singleton(Integer::from(*n)))),
            ExprKind::Lit(Lit::Char(c)) => {
                compile(TyKind::int(&Interval::singleton(Integer::from(i128::from(u32::from(*c))))))
            }
            ExprKind::Name(_) => self.name(expr),
            ExprKind::Member { object, name: Name::Plain(name) } => self.member(expr, *object, *name),
            ExprKind::StructLit { ty: Some(ty), fields } => {
                self.typed(*ty);
                let target = self.ty(*ty);
                self.struct_lit(target, fields)
            }
            ExprKind::StructLit { ty: None, fields } => self.struct_lit(self.expected(expr), fields),
            ExprKind::DotName(Name::Plain(name)) => self.dot(expr, *name),
            ExprKind::Tuple(items) => self.tuple(expr, items),
            ExprKind::Array(items) => self.array(expr, items),
            ExprKind::Repeat { value, count } => self.repeat(expr, *value, *count),
            ExprKind::Index { object, index } => self.index(*object, *index),
            // A type written in an expression, such as the argument of `Pair(u8)` or `[4]u8`.
            ExprKind::ArrayType { .. } => {
                visit::walk_expr(self, hir, expr);
                compile(TyKind::primitive(Primitive::Lang(Lang::r#type)))
            }
            ExprKind::Match { scrutinee, arms } => self.matching(expr, *scrutinee, arms),
            ExprKind::Return(value) => {
                if let Some(value) = value {
                    let found = self.typed_as(*value, self.result);
                    if let (Some(found), Some(result)) = (found, self.result) {
                        self.expect(hir.exprs[*value].origin, found, result, None);
                    }
                }
                None
            }
            ExprKind::Block(block) => self.block_value(*block, self.expected(expr)),
            ExprKind::Prefix { op: TokenKind::Dollar, operand, .. } => {
                let outer = std::mem::replace(&mut self.stage, Stage::Compile);
                let known = self.typed(*operand);
                self.stage = outer;
                known
            }
            ExprKind::Prefix { op: TokenKind::Bang, operand, .. } => self.logic(&[*operand]),
            ExprKind::Infix { op: TokenKind::And | TokenKind::Or, left, right: Some(right) } => {
                self.logic(&[*left, *right])
            }
            ExprKind::If { cond, then, otherwise } => self.branch(expr, cond, *then, *otherwise),
            ExprKind::For { head, body } => self.looping(None, head, *body),
            ExprKind::Labeled { label, body } => {
                let ExprKind::For { head, body: block } = &hir.exprs[*body].kind else {
                    visit::walk_expr(self, hir, expr);
                    return None;
                };
                let known = self.looping(Some(*label), head, *block);
                if let Some(known) = known {
                    self.types.insert(*body, known);
                }
                known
            }
            ExprKind::Postfix { op: TokenKind::DotStar, operand } => {
                let known = self.typed(*operand)?;
                Some(Known { ty: known.ty.pointee()?, ..known })
            }
            ExprKind::Break { label, value } => {
                self.leave(expr, *label, *value);
                None
            }
            ExprKind::Prefix { op: TokenKind::Minus, operand, .. } => {
                let known = self.typed(*operand)?;
                Some(Known { ty: TyKind::int(&known.ty.interval()?.neg()), ..known })
            }
            ExprKind::Infix { op: op @ (TokenKind::DotDot | TokenKind::DotDotEq), left, right } => {
                self.range(*op, *left, *right)
            }
            ExprKind::Infix { op, left, right: Some(right) } => {
                // The right operand is expected of the left's type: `c == .red`.
                let left = self.typed(*left);
                let right = self.typed_as(*right, left.map(|k| k.ty));
                self.binary(expr, *op, left?, right?)
            }
            ExprKind::Call { callee, args } => {
                if let (ExprKind::DotName(_), Some(expected)) =
                    (&hir.exprs[*callee].kind, self.expected(expr))
                {
                    self.expected.insert(*callee, expected);
                }
                self.call(expr, *callee, args)
            }
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

    /// A member of a namespace, of a type's namespace, or of a value: a field, or a declaration of
    /// its type, called as a method.
    fn member(&mut self, expr: ExprId, object: ExprId, name: Symbol) -> Option<Known> {
        let known = self.typed(object);
        // Only a path names a namespace or a type, so nothing else is evaluated here.
        if matches!(self.hir().exprs[object].kind, ExprKind::Name(_) | ExprKind::Member { .. }) {
            match self.env.eval(object) {
                Ok(Value::Namespace(ns)) => {
                    let site = self.env.cx.get::<Declared>(&(ns, name))?;
                    if let Site::Decl(decl) = &site
                        && let Some(visible) =
                            Env::of(self.env.cx, &decl.file).map(|e| e.decl_visible(decl.decl.id, name))
                    {
                        self.check_visible(expr, name, &visible);
                    }
                    return self.site(site);
                }
                Ok(Value::Type(ty)) => {
                    let members = self.env.cx.get::<Members>(&ty);
                    return match members.as_deref().map_or(Found::Unknown, |m| m.find(name)) {
                        Found::Decl(decl) => {
                            self.check_visible(expr, name, &decl.visible);
                            self.env.cx.get::<MemberType>(&(ty, name)).ok()
                        }
                        Found::Variant(v) if v.payload.is_empty() => {
                            Some(Known { ty, stage: Stage::Compile })
                        }
                        Found::Variant(_) | Found::Field(_) => None,
                        Found::Absent | Found::Unknown => {
                            let owner = self.method(expr, ty, name)?;
                            self.env.cx.get::<MemberType>(&(owner, name)).ok()
                        }
                    };
                }
                _ => {}
            }
        }
        let known = known?;
        let known = Known { ty: known.ty.borrowed().unwrap_or(known.ty), ..known };
        let members = self.env.cx.get::<Members>(&known.ty);
        match members.as_deref().map_or(Found::Unknown, |m| m.find(name)) {
            Found::Field(field) => {
                self.check_visible(expr, name, &field.visible);
                Some(Known { ty: *field.ty.as_ref().ok()?, stage: known.stage })
            }
            Found::Decl(decl) => {
                self.check_visible(expr, name, &decl.visible);
                self.methods.insert(expr, (known.ty, name));
                self.env.cx.get::<MemberType>(&(known.ty, name)).ok()
            }
            Found::Variant(_) => None,
            Found::Absent | Found::Unknown => {
                let owner = self.method(expr, known.ty, name)?;
                self.methods.insert(expr, (owner, name));
                self.env.cx.get::<MemberType>(&(owner, name)).ok()
            }
        }
    }

    /// The struct declaring the method `name` that `ty` has through an impl (spec 02, section 3).
    fn method(&mut self, expr: ExprId, ty: Ty, name: Symbol) -> Option<Ty> {
        let method = self.env.method(ty, name)?;
        self.check_visible(expr, name, &method.visible);
        Some(method.owner)
    }

    /// `T{ .name = value, ... }`, or `.{ ... }` of the expected type: each field's value is
    /// typed as the field and checked against it.
    fn struct_lit(&mut self, target: Option<Ty>, fields: &[Entry<Arg>]) -> Option<Known> {
        let members = target.and_then(|t| self.env.cx.get::<Members>(&t));
        let mut stage = Stage::Compile;
        for field in fields {
            let Entry::One(Arg { name, value }) = field else {
                stage = Stage::Run.max(self.stage);
                continue;
            };
            let at = self.hir().exprs[*value].origin;
            let found = match (&members, name) {
                (Some(members), Some(Name::Plain(name))) => match members.find(*name) {
                    Found::Field(field) => {
                        self.check_visible(*value, *name, &field.visible);
                        let expected = field.ty.as_ref().ok().copied();
                        let found = self.typed_as(*value, expected);
                        if let (Some(found), Some(expected)) = (found, expected) {
                            self.expect(at, found, expected, None);
                        }
                        found
                    }
                    Found::Absent | Found::Decl(_) | Found::Variant(_) => {
                        self.no_member(at, target?, *name);
                        self.typed(*value)
                    }
                    Found::Unknown => self.typed(*value),
                },
                _ => self.typed(*value),
            };
            stage = stage.max(found.map_or(self.stage, |k| k.stage));
        }
        Some(Known { ty: target?, stage })
    }

    pub(super) fn site(&self, site: Site) -> Option<Known> {
        match site {
            Site::Decl(decl) => self.env.cx.get::<DeclType>(&decl).ok(),
            Site::Namespace(_) => None,
        }
    }

    /// `left op right`: integer arithmetic by interval arithmetic (spec 04, rule 1), any other
    /// operator by the function of its `lang.ops` interface for the type of `left` (appendix B).
    /// A run-time intermediate past the width cap is reported (rule 5) and has no type, so
    /// nothing depending on it is.
    fn binary(&mut self, expr: ExprId, op: TokenKind, left: Known, right: Known) -> Option<Known> {
        let stage = left.stage.max(right.stage);
        if let (Some(a), Some(b)) = (left.ty.interval(), right.ty.interval())
            && let Some(range) = a.binary(op, &b)
        {
            if stage == Stage::Run && range.storage().is_none() && range.bits().is_some() {
                self.width_cap(expr, &range);
                return None;
            }
            return Some(Known { ty: TyKind::int(&range), stage });
        }
        let (_, interface, function) = OPERATORS.iter().find(|(token, ..)| *token == op)?;
        let Ok(Value::Interface(f)) = self.env.item(interface) else { return None };
        let requirements = self.env.cx.get::<Call>(&Instance { f, args: vec![Value::Type(left.ty)] });
        let key = (requirements.ok()?.as_type()?, Symbol::intern(function));
        let function = self.env.cx.get::<MemberType>(&key).ok()?;
        Some(Known { ty: function.ty.signature()?.result, stage })
    }
}
