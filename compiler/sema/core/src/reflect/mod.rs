//! Reflection (spec 03, section 6): types turned into plain data. Every value of a `struct` or
//! `enum` type is one variant of its type and the parts that variant holds; a struct is one
//! variant, its fields its parts. What a primitive gives is typed by its declaration in
//! `lang.meta`, so the descriptor structs are ordinary `lang` declarations whose fields the
//! compiler fills, each named by its handle.

use vun_hir::db::{Exports, Members as NamespaceMembers, Namespace};
use vun_intern::Symbol;
use vun_items::{Item, lang};
use vun_sema_int::Integer;
use vun_sema_ty::item::{Declared, Site};
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Ty, Value};

use crate::env::Env;
use crate::member::Members;

mod make;
mod params;
mod shape;

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

/// A member of a namespace: a declaration with the values of its attributes, or a namespace below.
struct Member {
    name: Symbol,
    attrs: Vec<Value>,
    nested: Option<Namespace>,
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

/// What reflection tells of a member of a namespace, by the descriptor field holding it.
const DECL: &[(Item, Fact<Member>)] = &[
    (lang::meta_Decl::name, |_, m| Ok(Value::string(m.name.as_str()))),
    (lang::meta_Decl::attrs, |_, m| Ok(Env::held(&m.attrs))),
    (lang::meta_Decl::nested, |env, m| {
        let option = env.field_type(lang::meta::Decl, lang::meta_Decl::nested)?;
        Ok(Env::optional(option, m.nested.clone().map(Value::Namespace)))
    }),
];

impl Env<'_, '_> {
    /// `lang.meta.decls(ns)`: a descriptor of each member of `ns` this file sees, every one
    /// within its package and the public ones from another.
    pub(crate) fn decls(&self, ns: &Namespace) -> Result<Value, Unevaluated> {
        let names = if ns.package == self.file.package {
            self.cx.get::<NamespaceMembers>(ns)
        } else {
            self.cx.get::<Exports>(ns)
        };
        let members = names.iter().filter_map(|&name| {
            let (attrs, nested) = match self.cx.get::<Declared>(&(ns.clone(), name))? {
                Site::Decl(decl) => {
                    let env = Env::of(self.cx, &decl.file)?;
                    (env.values(&env.parsed.hir.decls[decl.decl.id].attrs), None)
                }
                Site::Variant { .. } => (Vec::new(), None),
                Site::Namespace(inner) => (Vec::new(), Some(inner)),
            };
            Some(Member { name, attrs, nested })
        });
        self.describe(lang::meta::Decl, &members.collect::<Vec<_>>(), DECL)
    }

    /// The declared type of the field `field` of the `lang` struct `descriptor`.
    fn field_type(&self, descriptor: Item, field: Item) -> Result<Ty, Unevaluated> {
        let ty = self.item(descriptor)?.as_type().ok_or(Unevaluated::Unsupported)?;
        let members = self.cx.get::<Members>(&ty).ok_or(Unevaluated::Unsupported)?;
        let slot =
            members.fields.iter().find(|f| f.name == field.symbol()).ok_or(Unevaluated::Unsupported)?;
        slot.ty.clone()
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
