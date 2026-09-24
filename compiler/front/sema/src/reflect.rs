//! Reflection (spec 03, section 6): types turned into plain data. What a primitive gives is typed
//! by its declaration in `lang.meta`, so the descriptor structs are ordinary `lang` declarations
//! whose fields the compiler fills by name.

use crate::Primitive;
use crate::env::{Env, Unevaluated};
use crate::int::Integer;
use crate::member::{Field, Members, VariantDef};
use crate::ty::{Ty, Value};

/// How reflection computes one fact of what it describes.
type Fact<T> = fn(&T) -> Result<Value, Unevaluated>;

/// What reflection tells of a field, by the name of the descriptor field holding it.
const FIELD: &[(&str, Fact<Field>)] =
    &[("name", |f| Ok(Value::string(f.name.as_str()))), ("type", |f| f.ty.clone().map(Value::Type))];

/// What reflection tells of a variant, by the name of the descriptor field holding it.
const VARIANT: &[(&str, Fact<VariantDef>)] = &[
    ("name", |v| Ok(Value::string(v.name.as_str()))),
    ("payload", |v| {
        v.payload.iter().map(|t| t.clone().map(Value::Type)).collect::<Result<_, _>>().map(Value::Array)
    }),
];

impl Env<'_, '_> {
    /// The type of what `primitive` gives, as its declaration in `lang` states it.
    fn result_of(&self, primitive: Primitive) -> Result<Ty, Unevaluated> {
        Ok(self.primitive_signature(primitive, &[])?.result)
    }

    /// `lang.meta.fields(ty)`: a descriptor of each field of the struct type `ty`, of the item
    /// type of the slice `primitive` gives.
    pub(crate) fn fields(&self, primitive: Primitive, ty: Ty) -> Result<Value, Unevaluated> {
        let members = self.cx.get::<Members>(&ty).ok_or(Unevaluated::Unsupported)?;
        if !members.complete {
            return Err(Unevaluated::Unsupported);
        }
        self.describe(primitive, &members.fields, FIELD)
    }

    /// `lang.meta.variants(ty)`: a descriptor of each variant of the enum type `ty`, none for
    /// another type.
    pub(crate) fn variants(&self, primitive: Primitive, ty: Ty) -> Result<Value, Unevaluated> {
        let Some(members) = self.cx.get::<Members>(&ty) else {
            return self.describe(primitive, &[], VARIANT);
        };
        self.describe(primitive, &members.variants, VARIANT)
    }

    /// The position of the variant `value` among its enum type's.
    pub(crate) fn variant_index(&self, value: &Value) -> Result<Value, Unevaluated> {
        let Value::Variant { ty, name, .. } = value else { return Err(Unevaluated::Unsupported) };
        let members = self.cx.get::<Members>(ty).ok_or(Unevaluated::Unsupported)?;
        let at = members.variants.iter().position(|v| v.name == *name).ok_or(Unevaluated::Unsupported)?;
        Ok(Value::Int(Integer::from(at)))
    }

    /// A descriptor of each of `items`, of the item type of the slice `primitive` gives, each
    /// descriptor field filled by the fact of its name.
    fn describe<T>(
        &self,
        primitive: Primitive,
        items: &[T],
        facts: &[(&str, Fact<T>)],
    ) -> Result<Value, Unevaluated> {
        let result = self.result_of(primitive)?;
        let descriptor = result.borrowed().and_then(|s| s.items()).ok_or(Unevaluated::Unsupported)?;
        let layout = self.cx.get::<Members>(&descriptor).ok_or(Unevaluated::Unsupported)?;
        let described = items.iter().map(|item| {
            let values = layout.fields.iter().map(|slot| {
                let (_, fact) = facts
                    .iter()
                    .find(|(name, _)| *name == slot.name.as_str())
                    .ok_or(Unevaluated::Unsupported)?;
                Ok((slot.name, fact(item)?))
            });
            Ok(Value::Struct { ty: descriptor, fields: values.collect::<Result<_, _>>()? })
        });
        Ok(Value::Array(described.collect::<Result<_, _>>()?))
    }
}
