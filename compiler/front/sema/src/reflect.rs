//! Reflection (spec 03, section 6): types turned into plain data. What a primitive gives is typed
//! by its declaration in `lang.meta`, so the descriptor structs are ordinary `lang` declarations
//! whose fields the compiler fills by name.

use overt_intern::Symbol;

use crate::Primitive;
use crate::decl::DeclType;
use crate::env::{Env, Unevaluated};
use crate::item::{Declared, Site};
use crate::member::{Field, Members};
use crate::ty::{Ty, Value};

/// How reflection computes one fact of what it describes.
type Fact<T> = fn(&T) -> Result<Value, Unevaluated>;

/// What reflection tells of a field, by the name of the descriptor field holding it.
const FIELD: &[(&str, Fact<Field>)] =
    &[("name", |f| Ok(Value::string(f.name.as_str()))), ("type", |f| f.ty.clone().map(Value::Type))];

impl Env<'_, '_> {
    /// The type of what `primitive` gives, as its declaration in `lang` states it.
    fn result_of(&self, primitive: Primitive) -> Result<Ty, Unevaluated> {
        let Site::Namespace(ns) = self.site(primitive.namespace())? else {
            return Err(Unevaluated::Unsupported);
        };
        let site = self.cx.get::<Declared>(&(ns, Symbol::intern(primitive.name())));
        let Some(Site::Decl(decl)) = site else { return Err(Unevaluated::Absent) };
        let ty = self.cx.get::<DeclType>(&decl)?.ty;
        Ok(ty.signature().ok_or(Unevaluated::Unsupported)?.result)
    }

    /// `lang.meta.fields(ty)`: a descriptor of each field of the struct type `ty`, of the item
    /// type of the slice `primitive` gives.
    pub(crate) fn fields(&self, primitive: Primitive, ty: Ty) -> Result<Value, Unevaluated> {
        let result = self.result_of(primitive)?;
        let descriptor = result.borrowed().and_then(|s| s.items()).ok_or(Unevaluated::Unsupported)?;
        let members = self.cx.get::<Members>(&ty).ok_or(Unevaluated::Unsupported)?;
        if !members.complete {
            return Err(Unevaluated::Unsupported);
        }
        let layout = self.cx.get::<Members>(&descriptor).ok_or(Unevaluated::Unsupported)?;
        let described = members.fields.iter().map(|field| {
            let values = layout.fields.iter().map(|slot| {
                let (_, fact) = FIELD
                    .iter()
                    .find(|(name, _)| *name == slot.name.as_str())
                    .ok_or(Unevaluated::Unsupported)?;
                Ok((slot.name, fact(field)?))
            });
            Ok(Value::Struct { ty: descriptor, fields: values.collect::<Result<_, _>>()? })
        });
        Ok(Value::Array(described.collect::<Result<_, _>>()?))
    }
}
