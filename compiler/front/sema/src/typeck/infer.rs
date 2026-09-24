//! The type of each form typed so far. Any other form has its parts typed and no type itself.

use overt_grammar::{OPERATORS, TokenKind};
use overt_hir::resolve::Res;
use overt_hir::tree::{Arg, Entry, ExprId, ExprKind, Hir, Lit, Name};
use overt_hir::visit::{self, Visitor};
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
            ExprKind::Member { object, name: Name::Plain(name) } => self.member(expr, *object, *name),
            ExprKind::StructLit { ty: Some(ty), fields } => self.struct_lit(*ty, fields),
            ExprKind::Block(block) => self.block_value(*block),
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
            ExprKind::If { cond, then, otherwise } => self.branch(hir, cond, *then, *otherwise),
            ExprKind::Prefix { op: TokenKind::Minus, operand, .. } => {
                let known = self.typed(*operand)?;
                Some(Known { ty: TyKind::int(&known.ty.interval()?.neg()), ..known })
            }
            ExprKind::Infix { op, left, right: Some(right) } => {
                let (left, right) = (self.typed(*left), self.typed(*right));
                self.binary(expr, *op, left?, right?)
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
                            Env::of(self.env.cx, &decl.file).map(|e| e.decl_visible(decl.decl, name))
                    {
                        self.check_visible(expr, name, &visible);
                    }
                    return self.site(site);
                }
                Ok(Value::Type(ty)) => {
                    return match self.env.cx.get::<Members>(&ty)?.find(name) {
                        Found::Decl(decl) => {
                            self.check_visible(expr, name, &decl.visible);
                            self.env.cx.get::<MemberType>(&(ty, name)).ok()
                        }
                        Found::Variant(v) if v.payload.is_empty() => {
                            Some(Known { ty, stage: Stage::Compile })
                        }
                        _ => None,
                    };
                }
                _ => {}
            }
        }
        let known = known?;
        let known = Known { ty: known.ty.borrowed().unwrap_or(known.ty), ..known };
        let members = self.env.cx.get::<Members>(&known.ty)?;
        match members.find(name) {
            Found::Field(field) => {
                self.check_visible(expr, name, &field.visible);
                Some(Known { ty: *field.ty.as_ref().ok()?, stage: known.stage })
            }
            Found::Decl(decl) => {
                self.check_visible(expr, name, &decl.visible);
                self.methods.insert(expr);
                self.env.cx.get::<MemberType>(&(known.ty, name)).ok()
            }
            // Impls add methods to a type (spec 02, section 3), and they are not collected yet,
            // so a name missing from the struct is not known to be no member.
            Found::Absent | Found::Unknown | Found::Variant(_) => None,
        }
    }

    /// `T{ .name = value, ... }`: each field's value is checked against the field's type.
    fn struct_lit(&mut self, ty: ExprId, fields: &[Entry<Arg>]) -> Option<Known> {
        self.typed(ty);
        let target = self.env.ty(ty).ok();
        let members = target.and_then(|t| self.env.cx.get::<Members>(&t));
        let mut stage = Stage::Compile;
        for field in fields {
            let Entry::One(Arg { name, value }) = field else {
                stage = Stage::Run.max(self.stage);
                continue;
            };
            let found = self.typed(*value);
            stage = stage.max(found.map_or(self.stage, |k| k.stage));
            let (Some(members), Some(Name::Plain(name)), Some(found)) = (&members, name, found) else {
                continue;
            };
            let at = self.hir().exprs[*value].origin;
            match members.find(*name) {
                Found::Field(field) => {
                    self.check_visible(*value, *name, &field.visible);
                    if let Ok(expected) = field.ty {
                        self.expect(at, found, expected, None);
                    }
                }
                Found::Absent | Found::Decl(_) | Found::Variant(_) => self.no_member(at, target?, *name),
                Found::Unknown => {}
            }
        }
        Some(Known { ty: target?, stage })
    }

    fn site(&self, site: Site) -> Option<Known> {
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
        // A method's receiver is passed in the mode its first parameter asks for.
        let receiver = usize::from(self.methods.contains(&callee));
        let params = signature.params.get(receiver..)?;
        if params.len() != found.len() {
            return None;
        }
        for (&param, (value, known)) in params.iter().zip(found) {
            if let Some(known) = known {
                self.expect(self.hir().exprs[value].origin, known, param, None);
            }
        }
        Some(Known { ty: signature.result, stage: self.stage })
    }
}
