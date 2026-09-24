//! The type of each form typed so far. Any other form has its parts typed and no type itself.

use vun_grammar::TokenKind;
use vun_hir::tree::{Arg, Entry, ExprId, ExprKind, Hir, Lit, Name};
use vun_hir::visit;

use super::{Known, Stage, Typer};
use crate::Primitive;
use crate::decl::DeclType;
use crate::int::{Integer, Interval};
use crate::item::Site;
use crate::member::{Found, Members};
use crate::primitives::meta::Meta;
use crate::ty::{Ty, TyKind};

impl Typer<'_, '_, '_> {
    pub(super) fn infer(&mut self, hir: &Hir, expr: ExprId) -> Option<Known> {
        let compile = |ty| Some(Known { ty, stage: Stage::Compile });
        match &hir.exprs[expr].kind {
            ExprKind::Lit(Lit::Int(n)) => compile(TyKind::int(&Interval::singleton(Integer::from(*n)))),
            ExprKind::Lit(Lit::String(text)) => compile(TyKind::string(text.len())?),
            ExprKind::Lit(Lit::Char(c)) => {
                compile(TyKind::int(&Interval::singleton(Integer::from(i128::from(u32::from(*c))))))
            }
            ExprKind::Name(_) => self.name(expr),
            ExprKind::Member { object, name } => match (self.env().name(name), self.env().position(name)) {
                (Some(name), _) => self.member(expr, *object, name),
                (None, Some(at)) => self.position(expr, *object, at),
                (None, None) => self.typed(*object).and(None),
            },
            ExprKind::StructLit { ty: Some(ty), fields } => {
                self.typed(*ty);
                let target = self.ty(*ty);
                self.struct_lit(target, fields)
            }
            ExprKind::StructLit { ty: None, fields } => match self.expected(expr) {
                Some(expected) => self.struct_lit(Some(expected), fields),
                None => self.record_lit(fields),
            },
            ExprKind::DotName(Name::Plain(name)) => self.dot(expr, *name),
            ExprKind::Tuple(items) => self.tuple(expr, items),
            ExprKind::Array(items) => self.array(expr, items),
            ExprKind::Repeat { value, count } => self.repeat(expr, *value, *count),
            ExprKind::Index { object, index } => self.index(*object, *index),
            // A type written in an expression, such as the argument of `Pair(u8)` or `[4]u8`.
            ExprKind::ArrayType { .. } => {
                visit::walk_expr(self, hir, expr);
                compile(TyKind::of_types())
            }
            ExprKind::Match { scrutinee, arms } => self.matching(expr, *scrutinee, arms),
            ExprKind::Return(value) => {
                if let Some(value) = value {
                    let found = self.typed_as(*value, self.result);
                    if let (Some(found), Some(result)) = (found, self.result) {
                        self.expect(hir.exprs[*value].origin, found, result, self.written);
                    }
                }
                None
            }
            ExprKind::Block(block) => self.block_value(*block, self.expected(expr)),
            ExprKind::Prefix { op: TokenKind::Dollar, operand, .. } => self.comptime(expr, *operand),
            ExprKind::Prefix { op: TokenKind::Bang, operand, .. } => self.logic(&[*operand]),
            ExprKind::Prefix { op: TokenKind::Amp, mode, operand } => self.borrow(expr, *mode, *operand),
            ExprKind::Prefix { op: TokenKind::Unsafe, operand, .. } => self.vouching(expr, *operand),
            ExprKind::Attributed { attrs, inner } => self.attributed(expr, attrs, *inner),
            ExprKind::Infix { op: TokenKind::And | TokenKind::Or, left, right: Some(right) } => {
                self.logic(&[*left, *right])
            }
            ExprKind::If { cond, then, otherwise } => self.branch(expr, cond, *then, *otherwise),
            ExprKind::For { head, body } => self.looping(None, head, *body),
            ExprKind::Labeled { label, body } => self.labeled(expr, *label, *body),
            ExprKind::Postfix { op: TokenKind::Question, operand } => self.early_return(expr, *operand),
            ExprKind::Postfix { op: TokenKind::DotStar, operand } => {
                let known = self.typed(*operand)?;
                self.raw_deref(expr, known.ty);
                let owned = || self.env().cx.get::<Members>(&known.ty)?.deref().map(|(_, ty)| ty);
                Some(Known { ty: known.ty.pointee().or_else(owned)?, ..known })
            }
            ExprKind::Break { label, value } => {
                self.leave(expr, *label, *value);
                None
            }
            ExprKind::Prefix { op: op @ (TokenKind::Minus | TokenKind::Tilde), operand, .. } => {
                self.prefix(expr, *op, *operand)
            }
            ExprKind::Infix { op: op @ (TokenKind::DotDot | TokenKind::DotDotEq), left, right } => {
                self.range(*op, *left, *right)
            }
            ExprKind::Infix { op, left, right: Some(right) } => self.binary(expr, *op, *left, *right),
            ExprKind::Call { callee, args } if matches!(hir.exprs[*callee].kind, ExprKind::DotName(_)) => {
                self.dot_call(expr, *callee, args)
            }
            ExprKind::Call { callee, args } => self.call(expr, *callee, args),
            // A function literal is a value; its body is typed on its own.
            ExprKind::Closure { captures, func } => self.closure(expr, captures, *func),
            ExprKind::Fn { .. } => compile(self.env().literal_signature(expr, &[]).ok()?.ty()),
            ExprKind::Yield(value) => self.yielded(*value),
            ExprKind::FnType { .. } | ExprKind::Struct(_) | ExprKind::Enum { .. } => {
                compile(TyKind::of_types())
            }
            // `#{ e }` is code of the type of `e` (spec 03, section 4).
            ExprKind::Quote(block) => {
                let known = self.block_value(*block, None)?;
                Some(Known { ty: TyKind::code(known.ty), stage: Stage::Compile })
            }
            _ => {
                visit::walk_expr(self, hir, expr);
                None
            }
        }
    }

    /// `T{ .name = value, ... }`, or `.{ ... }` of the expected type: each field's value is
    /// typed as the field and checked against it.
    fn struct_lit(&mut self, target: Option<Ty>, fields: &[Entry<Arg>]) -> Option<Known> {
        let members = target.and_then(|t| self.env().cx.get::<Members>(&t));
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
            Site::Decl(decl) => self.env().cx.get::<DeclType>(&decl).ok(),
            // A file or directory is a value of `lang.meta.Namespace` (spec 01, section 8).
            Site::Namespace(_) => {
                Some(Known { ty: TyKind::primitive(Primitive::Meta(Meta::Namespace)), stage: Stage::Compile })
            }
        }
    }
}
