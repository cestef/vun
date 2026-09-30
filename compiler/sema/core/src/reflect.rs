//! Reflection (spec 03, section 6): types turned into plain data. Every value of a `struct` or
//! `enum` type is one variant of its type and the parts that variant holds; a struct is one
//! variant, its fields its parts. What a primitive gives is typed by its declaration in
//! `lang.meta`, so the descriptor structs are ordinary `lang` declarations whose fields the
//! compiler fills, each named by its handle.

use vun_intern::Symbol;
use vun_items::{Item, lang};
use vun_sema_int::Integer;
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Ty, TyKind, Value};

use crate::env::Env;
use crate::member::Members;

/// A part of a variant: a field of a struct, or an item of a payload, named by its position.
struct Part {
    name: Symbol,
    index: usize,
    ty: Result<Ty, Unevaluated>,
    attrs: Vec<Value>,
}

/// A variant of a type: an enum's, or the one a struct is, named by nothing.
struct Variant {
    name: Symbol,
    index: usize,
    parts: Vec<Part>,
    attrs: Vec<Value>,
}

/// How reflection computes one fact of what it describes.
pub(crate) type Fact<T> = fn(&Env<'_, '_>, &T) -> Result<Value, Unevaluated>;

/// What reflection tells of a part, by the descriptor field holding it.
const PART: &[(Item, Fact<Part>)] = &[
    (lang::meta_Field::name, |_, p| Ok(Value::string(p.name.as_str()))),
    (lang::meta_Field::index, |_, p| Ok(Value::Int(Integer::from(p.index)))),
    (lang::meta_Field::r#type, |_, p| p.ty.clone().map(Value::Type)),
    (lang::meta_Field::attrs, |_, p| Ok(Env::held(&p.attrs))),
];

/// What reflection tells of a variant, by the descriptor field holding it.
const VARIANT: &[(Item, Fact<Variant>)] = &[
    (lang::meta_Variant::name, |_, v| Ok(Value::string(v.name.as_str()))),
    (lang::meta_Variant::index, |_, v| Ok(Value::Int(Integer::from(v.index)))),
    (lang::meta_Variant::parts, |env, v| env.describe(lang::meta::Field, &v.parts, PART)),
    (lang::meta_Variant::attrs, |_, v| Ok(Env::held(&v.attrs))),
];

impl Env<'_, '_> {
    /// The variants of `ty`: an enum's, the one of a struct, none for another type. `None` while
    /// its members are not all known.
    fn shape(&self, ty: Ty) -> Result<Vec<Variant>, Unevaluated> {
        let Some(members) = self.cx.get::<Members>(&ty) else { return Ok(Vec::new()) };
        if !members.complete {
            return Err(Unevaluated::Unsupported);
        }
        if !matches!(&*ty, TyKind::Nominal(n) if n.kind == vun_sema_ty::NominalKind::Struct) {
            let variants = members.variants.iter().enumerate().map(|(index, v)| {
                let parts = v.payload.iter().enumerate();
                let parts = parts.map(|(i, ty)| Part {
                    name: Symbol::intern(&i.to_string()),
                    index: i,
                    ty: ty.clone(),
                    attrs: Vec::new(),
                });
                Variant { name: v.name, index, parts: parts.collect(), attrs: v.attrs.clone() }
            });
            return Ok(variants.collect());
        }
        let parts = members.fields.iter().enumerate();
        let parts =
            parts.map(|(index, f)| Part { name: f.name, index, ty: f.ty.clone(), attrs: f.attrs.clone() });
        Ok(vec![Variant { name: Symbol::intern(""), index: 0, parts: parts.collect(), attrs: Vec::new() }])
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
            Value::Struct { .. } => 0,
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
            Value::Variant { payload, .. }
                if self.variant_index(value)? == Value::Int(Integer::from(variant)) =>
            {
                payload.get(index).cloned()
            }
            Value::Struct { .. } | Value::Variant { .. } => None,
            _ => return Err(Unevaluated::Unsupported),
        };
        Ok(Self::optional(option, part))
    }

    /// `value` as a value of the option type `option`.
    fn optional(option: Ty, value: Option<Value>) -> Value {
        let (name, payload) = match value {
            Some(value) => (lang::Option::some.symbol(), vec![value]),
            None => (lang::Option::none.symbol(), Vec::new()),
        };
        Value::Variant { ty: option, name, payload }
    }

    /// `values` as a `&[lang.meta.Any]`, each with its type; one without a type, such as a
    /// function, is left out, as no `lang.meta.value` could give it back.
    fn held(values: &[Value]) -> Value {
        let held = values.iter().filter_map(|v| Some(Value::Any { ty: v.ty()?, value: Box::new(v.clone()) }));
        Value::Array(held.collect())
    }

    /// `lang.meta.value(T, a)`, of the option type `option`: what `a` holds, `.some` when it is
    /// of type `ty`.
    pub(crate) fn held_value(option: Ty, ty: Ty, any: &Value) -> Result<Value, Unevaluated> {
        let Value::Any { ty: held, value } = any else { return Err(Unevaluated::Unsupported) };
        Ok(Self::optional(option, (*held == ty).then(|| value.as_ref().clone())))
    }

    fn index_of(value: &Value) -> Result<usize, Unevaluated> {
        value.as_usize().ok_or(Unevaluated::Unsupported)
    }

    /// A descriptor of each of `items`, of the `lang` struct `descriptor`, each descriptor field
    /// filled by the fact of its handle.
    pub(crate) fn describe<T>(
        &self,
        descriptor: Item,
        items: &[T],
        facts: &[(Item, Fact<T>)],
    ) -> Result<Value, Unevaluated> {
        let descriptor = self.item(descriptor)?.as_type().ok_or(Unevaluated::Unsupported)?;
        let layout = self.cx.get::<Members>(&descriptor).ok_or(Unevaluated::Unsupported)?;
        let described = items.iter().map(|item| {
            let values = layout.fields.iter().map(|slot| {
                let (_, fact) =
                    facts.iter().find(|(f, _)| f.symbol() == slot.name).ok_or(Unevaluated::Unsupported)?;
                Ok((slot.name, fact(self, item)?))
            });
            Ok(Value::Struct { ty: descriptor, fields: values.collect::<Result<_, _>>()? })
        });
        Ok(Value::Array(described.collect::<Result<_, _>>()?))
    }
}
