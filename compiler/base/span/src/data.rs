//! Versions as data: their text form, `major.minor.patch`.

use vun_data::{Expected, FromData, Mismatch, ToData, Value};

use crate::Version;

impl ToData for Version {
    fn to_data(&self) -> Value<'static> {
        self.to_string().to_data()
    }
}

impl FromData for Version {
    fn from_data(value: &Value<'_>) -> Result<Self, Mismatch> {
        let text = value.as_str().ok_or(Mismatch::new(Expected::String))?;
        text.parse().map_err(|_| Mismatch::new(Expected::Syntax("a version, `major.minor.patch`")))
    }
}
