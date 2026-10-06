//! The declarations of a struct type, as reflection gives them: what an interface requires and
//! provides, each by its name, its type and whether its value is left to what implements it.

use vun_hir::tree::ExprKind;
use vun_items::lang;
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Nominal, Ty, TyKind, Value};

use super::{Declaration, Fact};
use crate::env::Env;
use crate::member::{MemberType, Members};

/// What reflection tells of a declaration of a struct, by the descriptor field holding it.
const DECLARATION: &[(vun_items::Item, Fact<Declaration>)] = &[
    (lang::meta_Member::name, |_, d| Ok(Value::string(d.name.as_str()))),
    (lang::meta_Member::r#type, |env, d| {
        let option = env.field_type(lang::meta::Member, lang::meta_Member::r#type)?;
        Ok(Env::optional(option, d.ty.map(Value::Type)))
    }),
    (lang::meta_Member::required, |env, d| env.decide(d.required)),
];

impl Env<'_, '_> {
    /// `lang.meta.members(ty)`: a descriptor of each declaration of the struct type `ty`, in
    /// declaration order. `Unsupported` while its members are not all known.
    pub(crate) fn members(&self, ty: Ty) -> Result<Value, Unevaluated> {
        let TyKind::Nominal(Nominal { literal, instance, .. }) = &*ty else {
            return self.describe(lang::meta::Member, &[], DECLARATION);
        };
        let table = self.cx.get::<Members>(&ty).ok_or(Unevaluated::Unsupported)?;
        if !table.complete {
            return Err(Unevaluated::Unsupported);
        }
        let env = Env::defining(self.cx, literal, instance.as_ref()).ok_or(Unevaluated::Absent)?;
        let declared = table.decls.iter().map(|decl| {
            let value = env.parsed.hir.decls[decl.decl.decl.id].value;
            Declaration {
                name: decl.name,
                ty: self.cx.get::<MemberType>(&(ty, decl.name)).ok().map(|known| known.ty),
                required: value.is_some_and(|v| matches!(env.parsed.hir.exprs[v].kind, ExprKind::Hole)),
            }
        });
        self.describe(lang::meta::Member, &declared.collect::<Vec<_>>(), DECLARATION)
    }
}
