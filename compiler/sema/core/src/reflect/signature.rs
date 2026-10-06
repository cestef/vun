//! The parts of a function type, as reflection gives them.

use vun_items::lang;
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Ty, TyKind, Value};

use super::Fact;
use crate::env::Env;

/// The parts of a function type, by the descriptor field holding each.
struct Parts {
    params: Ty,
    result: Ty,
}

const SIGNATURE: &[(vun_items::Item, Fact<Parts>)] = &[
    (lang::meta_Signature::Params, |_, p| Ok(Value::Type(p.params))),
    (lang::meta_Signature::Returns, |_, p| Ok(Value::Type(p.result))),
];

impl Env<'_, '_> {
    /// `lang.meta.signature(ty)`, of the option type `option`: the parts of the function type
    /// `ty`, none for another type.
    pub(crate) fn signature_of(&self, option: Ty, ty: Ty) -> Result<Value, Unevaluated> {
        let parts =
            ty.signature().map(|s| Parts { params: TyKind::Tuple(s.params).intern(), result: s.result });
        let described = match parts {
            Some(parts) => {
                let Value::Array(mut one) = self.describe(lang::meta::Signature, &[parts], SIGNATURE)? else {
                    return Err(Unevaluated::Unsupported);
                };
                one.pop()
            }
            None => None,
        };
        Ok(Self::optional(option, described))
    }
}
