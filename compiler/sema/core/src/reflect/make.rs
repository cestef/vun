//! Values made from parts, as reflection gives them: the variant of a type holding a tuple of its
//! parts, what `lang.meta.part` takes apart; a tuple is made of its items.

use vun_sema_ty::trap::{Reason, Unevaluated};
use vun_sema_ty::{Nominal, NominalKind, Ty, TyKind, Value};

use super::Variant;
use crate::env::Env;

impl Env<'_, '_> {
    /// `lang.meta.Parts(ty, variant)`: the tuple of the types of that variant's parts.
    pub(crate) fn parts_type(&self, ty: Ty, variant: &Value) -> Result<Value, Unevaluated> {
        let variant = self.variant_at(ty, variant)?;
        let parts = variant.parts.into_iter().map(|p| p.ty).collect::<Result<_, _>>()?;
        Ok(Value::Type(TyKind::Tuple(parts).intern()))
    }

    /// `lang.meta.make(ty, variant, parts)`: the value of `ty` that is that variant holding the
    /// items of the tuple `parts`.
    pub(crate) fn make(&self, ty: Ty, variant: &Value, parts: &Value) -> Result<Value, Unevaluated> {
        let Value::Tuple(items) = parts else { return Err(Unevaluated::Unsupported) };
        let variant = self.variant_at(ty, variant)?;
        Ok(match &*ty {
            TyKind::Nominal(Nominal { kind: NominalKind::Struct, .. }) | TyKind::Record(_) => {
                let names = variant.parts.iter().map(|p| p.name);
                Value::Struct { ty, fields: names.zip(items.iter().cloned()).collect() }
            }
            TyKind::Tuple(_) => Value::Tuple(items.clone()),
            _ => Value::Variant { ty, name: variant.name, payload: items.clone() },
        })
    }

    /// The variant at the position `variant` of `ty`; a position past its variants traps.
    fn variant_at(&self, ty: Ty, variant: &Value) -> Result<Variant, Unevaluated> {
        let at = Self::index_of(variant)?;
        self.shape(ty)?.into_iter().nth(at).ok_or_else(|| Unevaluated::trap(Reason::Argument))
    }
}
