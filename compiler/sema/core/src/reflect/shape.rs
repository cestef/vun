//! The one shape reflection gives every value of a `struct`, `enum`, tuple or record type: the
//! variants of its type and the parts each holds.

use vun_items::lang;
use vun_sema_int::Integer;
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Ty, TyKind, Value};

use super::{PART, Part, VARIANT, Variant};
use crate::env::Env;
use crate::member::Members;

impl Env<'_, '_> {
    /// The variants of `ty`: an enum's, the one a struct, a tuple or a record is, none for
    /// another type. `Unsupported` while its members are not all known.
    pub(super) fn shape(&self, ty: Ty) -> Result<Vec<Variant>, Unevaluated> {
        match &*ty {
            TyKind::Tuple(items) => {
                let parts = items.iter().enumerate().map(|(i, &item)| Part::at(i, Ok(item)));
                return Ok(vec![Variant::alone(parts.collect())]);
            }
            TyKind::Record(fields) => {
                let parts = fields.iter().enumerate();
                let parts =
                    parts.map(|(index, &(name, ty))| Part { name, index, ty: Ok(ty), attrs: Vec::new() });
                return Ok(vec![Variant::alone(parts.collect())]);
            }
            _ => {}
        }
        let Some(members) = self.cx.get::<Members>(&ty) else { return Ok(Vec::new()) };
        if !members.complete {
            return Err(Unevaluated::Unsupported);
        }
        if !matches!(&*ty, TyKind::Nominal(n) if n.kind == vun_sema_ty::NominalKind::Struct) {
            let codes = members.codes()?;
            let variants = members.variants.iter().enumerate().map(|(index, v)| {
                let parts = v.payload.iter().enumerate().map(|(i, ty)| Part::at(i, ty.clone()));
                let code = codes.as_ref().map_or_else(|| Integer::from(index), |codes| codes[index].clone());
                Variant { name: v.name, index, code, parts: parts.collect(), attrs: v.attrs.clone() }
            });
            return Ok(variants.collect());
        }
        let parts = members.fields.iter().enumerate();
        let parts =
            parts.map(|(index, f)| Part { name: f.name, index, ty: f.ty.clone(), attrs: f.attrs.clone() });
        Ok(vec![Variant::alone(parts.collect())])
    }

    /// The fields of the struct type `ty`, the parts of its one variant: `Cycle` while they are
    /// not known, as inside its own literal.
    fn struct_parts(&self, ty: Ty) -> Result<Vec<Part>, Unevaluated> {
        self.shape(ty)?.into_iter().next().map(|v| v.parts).ok_or(Unevaluated::Cycle)
    }

    /// `lang.meta.fields(ty)`: a descriptor of each field of the struct type `ty`.
    pub(crate) fn fields(&self, ty: Ty) -> Result<Value, Unevaluated> {
        let parts = match &*ty {
            TyKind::Nominal(n) if n.kind == vun_sema_ty::NominalKind::Struct => self.struct_parts(ty)?,
            _ => Vec::new(),
        };
        self.describe(lang::meta::Field, &parts, PART)
    }

    /// `lang.meta.variants(ty)`: a descriptor of each variant of `ty`.
    pub(crate) fn variants(&self, ty: Ty) -> Result<Value, Unevaluated> {
        self.describe(lang::meta::Variant, &self.shape(ty)?, VARIANT)
    }

    /// The position of the variant `value` is among its type's: 0 for a struct.
    pub(crate) fn variant_index(&self, value: &Value) -> Result<Value, Unevaluated> {
        let at = match value {
            Value::Struct { .. } | Value::Tuple(_) => 0,
            Value::Variant { ty, name, .. } => {
                let shape = self.shape(*ty)?;
                shape.iter().position(|v| v.name == *name).ok_or(Unevaluated::Unsupported)?
            }
            _ => return Err(Unevaluated::Unsupported),
        };
        Ok(Value::Int(Integer::from(at)))
    }

    /// `lang.meta.Part(ty, variant, index)`: the type of that part.
    pub(crate) fn part_type(&self, ty: Ty, variant: &Value, index: &Value) -> Result<Value, Unevaluated> {
        let (variant, index) = (Self::index_of(variant)?, Self::index_of(index)?);
        let shape = self.shape(ty)?;
        let part = shape.get(variant).and_then(|v| v.parts.get(index)).ok_or(Unevaluated::Unsupported)?;
        part.ty.clone().map(Value::Type)
    }

    /// `lang.meta.part(ty, value, variant, index)`: that part of `value`, `.some` when `value` is
    /// that variant, as an option of type `ty`.
    pub(crate) fn part(
        &self,
        option: Ty,
        value: &Value,
        variant: &Value,
        index: &Value,
    ) -> Result<Value, Unevaluated> {
        let (variant, index) = (Self::index_of(variant)?, Self::index_of(index)?);
        let part = match value {
            Value::Struct { ty, fields } if variant == 0 => {
                let name = self.struct_parts(*ty)?.get(index).ok_or(Unevaluated::Unsupported)?.name;
                fields.iter().find(|(n, _)| *n == name).map(|(_, v)| v.clone())
            }
            Value::Tuple(items) if variant == 0 => items.get(index).cloned(),
            Value::Variant { payload, .. }
                if self.variant_index(value)? == Value::Int(Integer::from(variant)) =>
            {
                payload.get(index).cloned()
            }
            Value::Struct { .. } | Value::Tuple(_) | Value::Variant { .. } => None,
            _ => return Err(Unevaluated::Unsupported),
        };
        Ok(Self::optional(option, part))
    }
}
