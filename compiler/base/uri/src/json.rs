//! References as JSON: a string holding one.

use vun_data::{Expected, FromData, Mismatch, ToData, Value};

use crate::uri::{Kind, UriOf};

impl<K: Kind> ToData for UriOf<K> {
    fn to_data(&self) -> Value<'static> {
        self.as_str().to_data()
    }
}

impl<K: Kind> FromData for UriOf<K> {
    fn from_data(value: &Value<'_>) -> Result<Self, Mismatch> {
        let text = value.as_str().ok_or(Mismatch::new(Expected::String))?;
        let expected = if K::SCHEME { "a URI" } else { "a URI reference" };
        text.parse::<Self>().map_err(|_| Mismatch::new(Expected::Syntax(expected)))
    }
}
