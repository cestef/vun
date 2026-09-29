//! The conversions of the standard types: scalars, text, sequences, options, maps and JSON itself.

use std::borrow::Cow;
use std::collections::BTreeMap;

use super::{Expected, FromJson, Mismatch, Step, ToJson};
use crate::{Number, Object, Value};

impl ToJson for bool {
    fn to_json(&self) -> Value<'static> {
        Value::Bool(*self)
    }
}

impl FromJson for bool {
    fn from_json(value: &Value<'_>) -> Result<Self, Mismatch> {
        value.as_bool().ok_or(Mismatch::new(Expected::Bool))
    }
}

macro_rules! integers {
    ($($t:ty),*) => {$(
        impl ToJson for $t {
            fn to_json(&self) -> Value<'static> {
                Value::from(*self)
            }
        }

        impl FromJson for $t {
            fn from_json(value: &Value<'_>) -> Result<Self, Mismatch> {
                let number = value.as_number().ok_or(Mismatch::new(Expected::Number))?;
                let fits = number.as_i64().and_then(|n| Self::try_from(n).ok());
                let fits = fits.or_else(|| number.as_u64().and_then(|n| Self::try_from(n).ok()));
                fits.ok_or(Mismatch::new(Expected::Integer))
            }
        }
    )*};
}

integers!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);

/// A number JSON cannot write, infinite or NaN, is `null`.
impl ToJson for f64 {
    fn to_json(&self) -> Value<'static> {
        Number::from_f64(*self).map_or(Value::Null, Value::Number)
    }
}

impl FromJson for f64 {
    fn from_json(value: &Value<'_>) -> Result<Self, Mismatch> {
        value.as_f64().ok_or(Mismatch::new(Expected::Number))
    }
}

impl ToJson for str {
    fn to_json(&self) -> Value<'static> {
        Value::from(self.to_owned())
    }
}

impl ToJson for String {
    fn to_json(&self) -> Value<'static> {
        self.as_str().to_json()
    }
}

impl FromJson for String {
    fn from_json(value: &Value<'_>) -> Result<Self, Mismatch> {
        value.as_str().map(str::to_owned).ok_or(Mismatch::new(Expected::String))
    }
}

impl ToJson for Cow<'_, str> {
    fn to_json(&self) -> Value<'static> {
        self.as_ref().to_json()
    }
}

impl FromJson for Cow<'static, str> {
    fn from_json(value: &Value<'_>) -> Result<Self, Mismatch> {
        String::from_json(value).map(Cow::Owned)
    }
}

impl<T: ToJson + ?Sized> ToJson for &T {
    fn to_json(&self) -> Value<'static> {
        (**self).to_json()
    }

    fn omitted(&self) -> bool {
        (**self).omitted()
    }
}

impl<T: ToJson + ?Sized> ToJson for Box<T> {
    fn to_json(&self) -> Value<'static> {
        (**self).to_json()
    }

    fn omitted(&self) -> bool {
        (**self).omitted()
    }
}

impl<T: FromJson> FromJson for Box<T> {
    fn from_json(value: &Value<'_>) -> Result<Self, Mismatch> {
        T::from_json(value).map(Box::new)
    }

    fn absent() -> Option<Self> {
        T::absent().map(Box::new)
    }
}

impl<T: ToJson> ToJson for [T] {
    fn to_json(&self) -> Value<'static> {
        Value::Array(self.iter().map(ToJson::to_json).collect())
    }
}

impl<T: ToJson> ToJson for Vec<T> {
    fn to_json(&self) -> Value<'static> {
        self.as_slice().to_json()
    }
}

impl<T: FromJson> FromJson for Vec<T> {
    fn from_json(value: &Value<'_>) -> Result<Self, Mismatch> {
        let items = value.as_array().ok_or(Mismatch::new(Expected::Array))?;
        items
            .iter()
            .enumerate()
            .map(|(i, item)| T::from_json(item).map_err(|m| m.within(Step::Item(i))))
            .collect()
    }
}

/// An option is `null` when read, and left out of its object when written absent.
impl<T: ToJson> ToJson for Option<T> {
    fn to_json(&self) -> Value<'static> {
        self.as_ref().map_or(Value::Null, ToJson::to_json)
    }

    fn omitted(&self) -> bool {
        self.is_none()
    }
}

impl<T: FromJson> FromJson for Option<T> {
    fn from_json(value: &Value<'_>) -> Result<Self, Mismatch> {
        if value.is_null() { Ok(None) } else { T::from_json(value).map(Some) }
    }

    fn absent() -> Option<Self> {
        Some(None)
    }
}

impl<T: ToJson> ToJson for BTreeMap<String, T> {
    fn to_json(&self) -> Value<'static> {
        let mut object = Object::new();
        for (name, value) in self {
            object.push(name.clone(), value.to_json());
        }
        Value::Object(object)
    }
}

impl<T: FromJson> FromJson for BTreeMap<String, T> {
    fn from_json(value: &Value<'_>) -> Result<Self, Mismatch> {
        let object = value.as_object().ok_or(Mismatch::new(Expected::Object))?;
        object.iter().map(|(name, value)| Ok((name.to_owned(), T::from_json(value)?))).collect()
    }
}

impl ToJson for () {
    fn to_json(&self) -> Value<'static> {
        Value::Null
    }
}

impl FromJson for () {
    fn from_json(value: &Value<'_>) -> Result<Self, Mismatch> {
        if value.is_null() { Ok(()) } else { Err(Mismatch::new(Expected::Null)) }
    }
}

impl ToJson for Value<'_> {
    fn to_json(&self) -> Value<'static> {
        self.clone().into_owned()
    }
}

/// Any value, as it is; absent, `null`.
impl FromJson for Value<'static> {
    fn from_json(value: &Value<'_>) -> Result<Self, Mismatch> {
        Ok(value.clone().into_owned())
    }

    fn absent() -> Option<Self> {
        Some(Value::Null)
    }
}

impl ToJson for Object<'_> {
    fn to_json(&self) -> Value<'static> {
        Value::Object(self.clone().into_owned())
    }
}

impl FromJson for Object<'static> {
    fn from_json(value: &Value<'_>) -> Result<Self, Mismatch> {
        value.as_object().map(|o| o.clone().into_owned()).ok_or(Mismatch::new(Expected::Object))
    }
}
