//! References as JSON: a string holding one.

use vun_json::{Expected, FromJson, Mismatch, ToJson, Value};

use crate::uri::{Kind, UriOf};

impl<K: Kind> ToJson for UriOf<K> {
    fn to_json(&self) -> Value<'static> {
        self.as_str().to_json()
    }
}

impl<K: Kind> FromJson for UriOf<K> {
    fn from_json(value: &Value<'_>) -> Result<Self, Mismatch> {
        let text = value.as_str().ok_or(Mismatch::new(Expected::String))?;
        let expected = if K::SCHEME { "a URI" } else { "a URI reference" };
        Self::parse(text).map_err(|_| Mismatch::new(Expected::Syntax(expected)))
    }
}
