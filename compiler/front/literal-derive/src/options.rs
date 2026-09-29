//! What the `literal` attributes and the item's shape decide: the form of each case, and its
//! name as the literal writes it.

use vun_derive_core::{Attrs, Body, Case, Casing, Field, Fields, Given, Item, On, Unsupported};

/// The item with the forms of its cases.
pub(crate) struct Literal<'i> {
    pub(crate) item: &'i Item,
    pub(crate) shapes: Vec<Shape<'i>>,
}

/// How a case is written.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Form {
    /// A struct's record of fields.
    Record,
    /// A struct of one field, read as it.
    Newtype,
    /// `.name`
    Name,
    /// `.name(values...)`, the last field taking the rest if marked.
    Tagged { rest: bool },
    /// Any value its one field reads.
    Untagged,
}

pub(crate) struct Shape<'i> {
    pub(crate) case: Case<'i>,
    pub(crate) form: Form,
    pub(crate) name: String,
    /// Each field with its name as the literal writes it.
    pub(crate) fields: Vec<(&'i Field, String)>,
}

/// The arguments of `#[literal(...)]`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Key {
    Name,
    Rest,
    Untagged,
}

const KEYS: [vun_derive_core::Key<Key>; 3] = [
    (Key::Name, "name", &[On::Variant, On::Field]),
    (Key::Rest, "rest", &[On::Field]),
    (Key::Untagged, "untagged", &[On::Variant]),
];

const HELPER: &str = "literal";

fn given(attrs: &Attrs, on: On) -> Result<Given<Key>, Unsupported> {
    Given::of(attrs, HELPER, &KEYS, on)
}

impl<'i> Literal<'i> {
    pub(crate) fn of(item: &'i Item) -> Result<Self, Unsupported> {
        given(&item.attrs, On::Item)?;
        let shapes = item.cases().into_iter().map(|case| Shape::of(case, &item.body));
        Ok(Self { item, shapes: shapes.collect::<Result<_, _>>()? })
    }
}

impl<'i> Shape<'i> {
    fn of(case: Case<'i>, body: &Body) -> Result<Self, Unsupported> {
        let fields = case.fields.all();
        let named = fields.iter().map(|field| {
            let given = given(&field.attrs, On::Field)?;
            Ok((field, given.name(Key::Name, &field.binding.to_string(), Casing::Snake)?))
        });
        let named: Vec<_> = named.collect::<Result<_, _>>()?;
        let rest = fields.iter().map(|f| given(&f.attrs, On::Field)).collect::<Result<Vec<_>, _>>()?;
        let rest_at = rest.iter().position(|g| g.has(Key::Rest));
        if rest_at.is_some_and(|at| at + 1 != fields.len()) {
            return Err(Unsupported("`rest` is for a variant's last field"));
        }
        let (form, name) = match (body, case.variant) {
            (Body::Struct(Fields::Named(_)), _) => (Form::Record, String::new()),
            (Body::Struct(Fields::Tuple(one)), _) if one.len() == 1 => (Form::Newtype, String::new()),
            (Body::Struct(_), _) => return Err(Unsupported("a struct has named fields, or one unnamed")),
            (Body::Enum(_), Some(variant)) => {
                let given = given(&variant.attrs, On::Variant)?;
                let name = given.name(Key::Name, &variant.name.to_string(), Casing::Snake)?;
                let form = match &variant.fields {
                    Fields::Unit => Form::Name,
                    Fields::Tuple(one) if given.has(Key::Untagged) && one.len() == 1 => Form::Untagged,
                    Fields::Tuple(_) if !given.has(Key::Untagged) => Form::Tagged { rest: rest_at.is_some() },
                    _ => {
                        return Err(Unsupported(
                            "a variant is a unit, or of unnamed fields; untagged, of one",
                        ));
                    }
                };
                (form, name)
            }
            (Body::Enum(_), None) => unreachable!("an enum's cases are its variants"),
        };
        if rest_at.is_some() && !matches!(form, Form::Tagged { .. }) {
            return Err(Unsupported("`rest` is for a tagged variant's last field"));
        }
        Ok(Self { case, form, name, fields: named })
    }
}
