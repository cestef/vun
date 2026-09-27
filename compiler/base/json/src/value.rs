//! Values, with accessors that answer `None` rather than panic, and indexing that reads a missing
//! member or item as `null`, so that a path through a document is one expression.

use std::borrow::Cow;
use std::ops::Index;

use crate::Number;

/// A JSON value. Strings and numbers borrow from the text read where they can.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Value<'a> {
    #[default]
    Null,
    Bool(bool),
    Number(Number<'a>),
    String(Cow<'a, str>),
    Array(Vec<Value<'a>>),
    Object(Object<'a>),
}

static NULL: Value<'static> = Value::Null;

/// An object's members in the order written. Names are compared as text; the RFC leaves duplicate
/// names to the reader, and this one keeps each, with [`Object::get`] answering the last.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Object<'a> {
    members: Vec<(Cow<'a, str>, Value<'a>)>,
}

impl<'a> Object<'a> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The value of the last member named `name`.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Value<'a>> {
        self.members.iter().rev().find(|(n, _)| n == name).map(|(_, v)| v)
    }

    /// Appends a member, keeping an earlier one of the same name.
    pub fn push(&mut self, name: impl Into<Cow<'a, str>>, value: impl Into<Value<'a>>) {
        self.members.push((name.into(), value.into()));
    }

    /// This object with a member appended: `Object::new().with("id", 3).with("name", "x")`.
    #[must_use]
    pub fn with(mut self, name: impl Into<Cow<'a, str>>, value: impl Into<Value<'a>>) -> Self {
        self.push(name, value);
        self
    }

    /// The members, in order.
    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&str, &Value<'a>)> {
        self.members.iter().map(|(n, v)| (n.as_ref(), v))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.members.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    #[must_use]
    pub fn into_owned(self) -> Object<'static> {
        let members = self.members.into_iter().map(|(n, v)| (Cow::Owned(n.into_owned()), v.into_owned()));
        Object { members: members.collect() }
    }
}

impl<'a> From<Vec<(Cow<'a, str>, Value<'a>)>> for Object<'a> {
    fn from(members: Vec<(Cow<'a, str>, Value<'a>)>) -> Self {
        Self { members }
    }
}

impl<'a> IntoIterator for Object<'a> {
    type Item = (Cow<'a, str>, Value<'a>);
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.members.into_iter()
    }
}

impl<'a> Value<'a> {
    #[must_use]
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_number(&self) -> Option<&Number<'a>> {
        match self {
            Self::Number(n) => Some(n),
            _ => None,
        }
    }

    /// The value of an integer number that fits.
    #[must_use]
    pub fn as_u64(&self) -> Option<u64> {
        self.as_number()?.as_u64()
    }

    /// The value of an integer number that fits.
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        self.as_number()?.as_i64()
    }

    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        Some(self.as_number()?.as_f64())
    }

    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_array(&self) -> Option<&[Value<'a>]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_object(&self) -> Option<&Object<'a>> {
        match self {
            Self::Object(o) => Some(o),
            _ => None,
        }
    }

    /// The member `name` of an object.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Value<'a>> {
        self.as_object()?.get(name)
    }

    /// The items of an array, none for any other value: `for node in doc["inner"].items()`.
    pub fn items(&self) -> std::slice::Iter<'_, Value<'a>> {
        self.as_array().unwrap_or_default().iter()
    }

    /// The value a JSON Pointer (RFC 6901) names, such as `/inner/0/name`; `""` is the whole.
    #[must_use]
    pub fn pointer(&self, pointer: &str) -> Option<&Value<'a>> {
        if pointer.is_empty() {
            return Some(self);
        }
        let mut at = self;
        for token in pointer.strip_prefix('/')?.split('/') {
            let token = token.replace("~1", "/").replace("~0", "~");
            at = match at {
                Self::Object(o) => o.get(&token)?,
                // An index is a canonical decimal: no sign and no leading zero.
                Self::Array(items) if token == "0" || !token.starts_with('0') => {
                    items.get(token.parse::<usize>().ok()?)?
                }
                _ => return None,
            };
        }
        Some(at)
    }

    /// This value, owning all its text, so that it outlives what it was read from.
    #[must_use]
    pub fn into_owned(self) -> Value<'static> {
        match self {
            Self::Null => Value::Null,
            Self::Bool(b) => Value::Bool(b),
            Self::Number(n) => Value::Number(n.into_owned()),
            Self::String(s) => Value::String(Cow::Owned(s.into_owned())),
            Self::Array(items) => Value::Array(items.into_iter().map(Value::into_owned).collect()),
            Self::Object(o) => Value::Object(o.into_owned()),
        }
    }
}

impl<'a> Index<&str> for Value<'a> {
    type Output = Value<'a>;

    /// The member `name`, or `null` when this is no object or has no such member.
    fn index(&self, name: &str) -> &Self::Output {
        self.get(name).unwrap_or(&NULL)
    }
}

impl<'a> Index<usize> for Value<'a> {
    type Output = Value<'a>;

    /// The item at `i`, or `null` when this is no array or is shorter.
    fn index(&self, i: usize) -> &Self::Output {
        self.as_array().and_then(|items| items.get(i)).unwrap_or(&NULL)
    }
}

impl From<bool> for Value<'_> {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

impl<'a> From<&'a str> for Value<'a> {
    fn from(s: &'a str) -> Self {
        Self::String(Cow::Borrowed(s))
    }
}

impl From<String> for Value<'_> {
    fn from(s: String) -> Self {
        Self::String(Cow::Owned(s))
    }
}

impl<'a> From<Number<'a>> for Value<'a> {
    fn from(n: Number<'a>) -> Self {
        Self::Number(n)
    }
}

impl<'a> From<Object<'a>> for Value<'a> {
    fn from(o: Object<'a>) -> Self {
        Self::Object(o)
    }
}

impl<'a, T: Into<Value<'a>>> From<Vec<T>> for Value<'a> {
    fn from(items: Vec<T>) -> Self {
        Self::Array(items.into_iter().map(Into::into).collect())
    }
}

impl<'a, T: Into<Value<'a>>> From<Option<T>> for Value<'a> {
    /// `null` for `None`.
    fn from(value: Option<T>) -> Self {
        value.map_or(Self::Null, Into::into)
    }
}

macro_rules! from_integer {
    ($($t:ty),*) => {$(
        impl From<$t> for Value<'_> {
            fn from(n: $t) -> Self {
                Self::Number(n.into())
            }
        }
    )*};
}
from_integer!(u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize);
