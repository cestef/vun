//! The conversions of the standard types: scalars, text, sequences, options, maps and JSON itself.

use std::borrow::Cow;
use std::collections::BTreeMap;

use super::{Expected, FromData, Mismatch, Step, ToData};
use crate::{Number, Object, Value};

impl ToData for bool {
    fn to_data(&self) -> Value<'static> {
        Value::Bool(*self)
    }
}

impl FromData for bool {
    fn from_data(value: &Value<'_>) -> Result<Self, Mismatch> {
        value.as_bool().ok_or(Mismatch::new(Expected::Bool))
    }
}

macro_rules! integers {
    ($($t:ty),*) => {$(
        impl ToData for $t {
            fn to_data(&self) -> Value<'static> {
                Value::from(*self)
            }
        }

        impl FromData for $t {
            fn from_data(value: &Value<'_>) -> Result<Self, Mismatch> {
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
impl ToData for f64 {
    fn to_data(&self) -> Value<'static> {
        Number::from_f64(*self).map_or(Value::Null, Value::Number)
    }
}

impl FromData for f64 {
    fn from_data(value: &Value<'_>) -> Result<Self, Mismatch> {
        value.as_f64().ok_or(Mismatch::new(Expected::Number))
    }
}

impl ToData for str {
    fn to_data(&self) -> Value<'static> {
        Value::from(self.to_owned())
    }
}

impl ToData for String {
    fn to_data(&self) -> Value<'static> {
        self.as_str().to_data()
    }
}

impl FromData for String {
    fn from_data(value: &Value<'_>) -> Result<Self, Mismatch> {
        value.as_str().map(str::to_owned).ok_or(Mismatch::new(Expected::String))
    }
}

impl ToData for Cow<'_, str> {
    fn to_data(&self) -> Value<'static> {
        self.as_ref().to_data()
    }
}

impl FromData for Cow<'static, str> {
    fn from_data(value: &Value<'_>) -> Result<Self, Mismatch> {
        String::from_data(value).map(Cow::Owned)
    }
}

impl<T: ToData + ?Sized> ToData for &T {
    fn to_data(&self) -> Value<'static> {
        (**self).to_data()
    }

    fn omitted(&self) -> bool {
        (**self).omitted()
    }
}

impl<T: ToData + ?Sized> ToData for Box<T> {
    fn to_data(&self) -> Value<'static> {
        (**self).to_data()
    }

    fn omitted(&self) -> bool {
        (**self).omitted()
    }
}

impl<T: FromData> FromData for Box<T> {
    fn from_data(value: &Value<'_>) -> Result<Self, Mismatch> {
        T::from_data(value).map(Box::new)
    }

    fn absent() -> Option<Self> {
        T::absent().map(Box::new)
    }
}

impl<T: ToData> ToData for [T] {
    fn to_data(&self) -> Value<'static> {
        Value::Array(self.iter().map(ToData::to_data).collect())
    }
}

impl<T: ToData> ToData for Vec<T> {
    fn to_data(&self) -> Value<'static> {
        self.as_slice().to_data()
    }
}

impl<T: FromData> FromData for Vec<T> {
    fn from_data(value: &Value<'_>) -> Result<Self, Mismatch> {
        let items = value.as_array().ok_or(Mismatch::new(Expected::Array))?;
        items
            .iter()
            .enumerate()
            .map(|(i, item)| T::from_data(item).map_err(|m| m.within(Step::Item(i))))
            .collect()
    }
}

/// An option is `null` when read, and left out of its object when written absent.
impl<T: ToData> ToData for Option<T> {
    fn to_data(&self) -> Value<'static> {
        self.as_ref().map_or(Value::Null, ToData::to_data)
    }

    fn omitted(&self) -> bool {
        self.is_none()
    }
}

impl<T: FromData> FromData for Option<T> {
    fn from_data(value: &Value<'_>) -> Result<Self, Mismatch> {
        if value.is_null() { Ok(None) } else { T::from_data(value).map(Some) }
    }

    fn absent() -> Option<Self> {
        Some(None)
    }
}

impl<T: ToData> ToData for BTreeMap<String, T> {
    fn to_data(&self) -> Value<'static> {
        let mut object = Object::new();
        for (name, value) in self {
            object.push(name.clone(), value.to_data());
        }
        Value::Object(object)
    }
}

impl<T: FromData> FromData for BTreeMap<String, T> {
    fn from_data(value: &Value<'_>) -> Result<Self, Mismatch> {
        let object = value.as_object().ok_or(Mismatch::new(Expected::Object))?;
        object.iter().map(|(name, value)| Ok((name.to_owned(), T::from_data(value)?))).collect()
    }
}

impl ToData for () {
    fn to_data(&self) -> Value<'static> {
        Value::Null
    }
}

impl FromData for () {
    fn from_data(value: &Value<'_>) -> Result<Self, Mismatch> {
        if value.is_null() { Ok(()) } else { Err(Mismatch::new(Expected::Null)) }
    }
}

impl ToData for Value<'_> {
    fn to_data(&self) -> Value<'static> {
        self.clone().into_owned()
    }
}

/// Any value, as it is; absent, `null`.
impl FromData for Value<'static> {
    fn from_data(value: &Value<'_>) -> Result<Self, Mismatch> {
        Ok(value.clone().into_owned())
    }

    fn absent() -> Option<Self> {
        Some(Value::Null)
    }
}

impl ToData for Object<'_> {
    fn to_data(&self) -> Value<'static> {
        Value::Object(self.clone().into_owned())
    }
}

impl FromData for Object<'static> {
    fn from_data(value: &Value<'_>) -> Result<Self, Mismatch> {
        value.as_object().map(|o| o.clone().into_owned()).ok_or(Mismatch::new(Expected::Object))
    }
}
