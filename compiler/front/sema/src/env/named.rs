//! The values of names: bindings of the environment, `$` locals, and declarations, a member of a
//! `struct` literal written inside a compile-time call being the member of the type that call
//! made; and tuple positions written in a name position.

use vun_hir::resolve::Res;
use vun_hir::tree::{BinderId, ExprId, Name, PatKind};
use vun_origin::OriginTable;

use super::{Env, Unevaluated};
use crate::eval::Instance;
use crate::layout::Exists;
use crate::member::MemberValue;
use crate::ty::{NominalKind, Ty, Value};

impl Env<'_, '_> {
    /// The value of the name `expr`: a binding of this environment, a `$` local fixed by its
    /// declaration, a member of a type a call here made, or what the name resolves to.
    pub(super) fn named(&self, expr: ExprId) -> Result<Value, Unevaluated> {
        let hir = &self.parsed.hir;
        match self.parsed.resolution.names[expr] {
            Some(Res::Binder(b)) if let Some(bound) = self.bindings.get(&b) => Ok(bound.clone()),
            // A `mut $` local changes as statements run, which they do only inside a call.
            Some(Res::Binder(b)) if hir.binders[b].comptime && hir.binders[b].mutable => {
                Err(Unevaluated::Unsupported)
            }
            // A `$` local that is never assigned has the value of its declaration, fixed by
            // the call evaluated in, such as `$Regs := struct { ... }`; only a declaration
            // binding the name alone holds the whole value.
            Some(Res::Binder(b)) if hir.binders[b].comptime && hir.member_decl(b).is_none() => {
                self.eval(self.declared(b).ok_or(Unevaluated::Runtime)?)
            }
            // So does any local holding a value that exists at compile time only, such as
            // `arith := lang.float.soft.arith`.
            Some(Res::Binder(b)) if let Some(value) = self.declared(b) => {
                let value = self.eval(value)?;
                if self.compile_only(&value) { Ok(value) } else { Err(Unevaluated::Runtime) }
            }
            Some(Res::Binder(b)) if let Some(member) = self.made_member(b) => member,
            _ => self.value_of(self.resolve(expr)?),
        }
    }

    /// The value of the declaration binding `binder` alone and for good: what a local known at
    /// compile time holds.
    #[must_use]
    pub fn declared(&self, binder: BinderId) -> Option<ExprId> {
        let hir = &self.parsed.hir;
        if hir.binders[binder].mutable || hir.member_decl(binder).is_some() {
            return None;
        }
        let decl = hir.decl_of(binder)?;
        let whole = matches!(hir.pats[hir.decls[decl].pat].kind, PatKind::Bind(b) if b == binder);
        hir.decls[decl].value.filter(|_| whole)
    }

    /// Whether `value` exists at compile time only: a namespace, a type, a name.
    #[must_use]
    pub fn compile_only(&self, value: &Value) -> bool {
        value.ty().is_some_and(|ty| self.compile_only_type(ty))
    }

    /// Whether values of `ty` exist at compile time only.
    #[must_use]
    pub fn compile_only_type(&self, ty: Ty) -> bool {
        !self.cx.get::<Exists>(&ty)
    }

    /// The tuple position a member name writes: `.0`, or `.$(i)` with `i` a known integer.
    #[must_use]
    pub fn position(&self, name: &Name) -> Option<usize> {
        match name {
            Name::Index(at) => usize::try_from(*at).ok(),
            Name::Computed(expr) => self.eval(*expr).ok()?.as_usize(),
            Name::Plain(_) | Name::Hole => None,
        }
    }

    /// The member `binder` of a `struct` literal inside the function a call of this environment
    /// runs, as the type that call made holds it: `G` in a method of `M(u8)` is `M(u8).G`, not
    /// a `G` of no call. `None` when no such call encloses the literal.
    fn made_member(&self, binder: BinderId) -> Option<Result<Value, Unevaluated>> {
        let hir = &self.parsed.hir;
        let literal = hir.struct_of(hir.member_ns(binder)?)?;
        let calls = std::iter::successors(self.instance.as_ref(), |i| i.f.within.as_deref());
        let call = calls.filter(|i| i.f.literal.file == self.file).find(|i| self.runs_around(i, literal))?;
        let made = Env::within(self.cx, call)?.nominal(literal, NominalKind::Struct);
        let ty = made.as_type()?;
        Some(self.cx.get::<MemberValue>(&(ty, hir.binders[binder].name)))
    }

    /// Whether the function `call` runs is written around `expr`.
    fn runs_around(&self, call: &Instance, expr: ExprId) -> bool {
        let origins = OriginTable::global();
        let span = |e: ExprId| origins.source_span(self.parsed.hir.exprs[e].origin);
        let (outer, inner) = (span(call.f.literal.expr.id), span(expr));
        outer.file == inner.file && outer.range.contains(inner.range)
    }
}
