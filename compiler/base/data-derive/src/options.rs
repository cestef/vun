//! What the `data` attributes and the item's shape decide: the form of its JSON, and the wire name
//! of each variant and field.

use vun_derive_core::{Attrs, Body, Case, Casing, Field, Fields, Given, Item, On, Unsupported};

/// The item with what its JSON is.
pub(crate) struct Data<'i> {
    pub(crate) item: &'i Item,
    pub(crate) form: Form,
    pub(crate) shapes: Vec<Shape<'i>>,
}

/// The form of the item's JSON.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Form {
    /// A struct: its fields' form.
    Struct,
    /// An enum of unit variants, each a string.
    Names,
    /// An enum of unit variants, each its discriminant.
    Numbers,
    /// An enum, each variant its fields' form.
    Untagged,
}

/// A case of the item, with the name of its variant and its members on the wire.
pub(crate) struct Shape<'i> {
    pub(crate) case: Case<'i>,
    pub(crate) name: String,
    pub(crate) members: Vec<Member<'i>>,
}

pub(crate) struct Member<'i> {
    pub(crate) field: &'i Field,
    pub(crate) name: String,
    pub(crate) flatten: bool,
}

/// The arguments of `#[data(...)]`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Key {
    Case,
    Untagged,
    Number,
    Flatten,
    Name,
}

const KEYS: [vun_derive_core::Key<Key>; 5] = [
    (Key::Case, "case", &[On::Item]),
    (Key::Untagged, "untagged", &[On::Item]),
    (Key::Number, "number", &[On::Item]),
    (Key::Flatten, "flatten", &[On::Field]),
    (Key::Name, "name", &[On::Variant, On::Field]),
];

const HELPER: &str = "data";

fn given(attrs: &Attrs, on: On) -> Result<Given<Key>, Unsupported> {
    Given::of(attrs, HELPER, &KEYS, on)
}

impl<'i> Data<'i> {
    pub(crate) fn of(item: &'i Item) -> Result<Self, Unsupported> {
        let given = given(&item.attrs, On::Item)?;
        let casing = match given.value(Key::Case) {
            Some(value) => {
                Casing::named(&value.to_string()).ok_or(Unsupported("`case` is `snake` or `camel`"))?
            }
            None if given.has(Key::Case) => return Err(Unsupported("`case` takes a casing")),
            None => Casing::default(),
        };
        let units = match &item.body {
            Body::Enum(variants) => variants.iter().all(|v| matches!(v.fields, Fields::Unit)),
            Body::Struct(_) => false,
        };
        let form = match (&item.body, given.has(Key::Untagged), given.has(Key::Number)) {
            (Body::Struct(_), false, false) => Form::Struct,
            (Body::Enum(_), true, false) => Form::Untagged,
            (Body::Enum(_), false, true) if units => Form::Numbers,
            (Body::Enum(_), false, false) if units => Form::Names,
            (Body::Enum(_), false, false) => return Err(Unsupported("an enum with fields is `untagged`")),
            _ => return Err(Unsupported("`untagged` and `number` are for enums, `number` of unit variants")),
        };
        let shapes = item.cases().into_iter().map(|case| Shape::of(case, item, casing));
        Ok(Self { item, form, shapes: shapes.collect::<Result<_, _>>()? })
    }
}

impl<'i> Shape<'i> {
    fn of(case: Case<'i>, item: &Item, casing: Casing) -> Result<Self, Unsupported> {
        let name = match case.variant {
            Some(variant) => {
                given(&variant.attrs, On::Variant)?.name(Key::Name, &variant.name.to_string(), casing)?
            }
            None => item.name.to_string(),
        };
        let members = case.fields.all().iter().map(|field| {
            let given = given(&field.attrs, On::Field)?;
            let flatten = given.has(Key::Flatten);
            if flatten && !matches!(case.fields, Fields::Named(_)) {
                return Err(Unsupported("`flatten` is for named fields"));
            }
            Ok(Member { field, name: given.name(Key::Name, &field.binding.to_string(), casing)?, flatten })
        });
        Ok(Self { members: members.collect::<Result<_, _>>()?, name, case })
    }
}
