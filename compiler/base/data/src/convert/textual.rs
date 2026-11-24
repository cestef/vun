//! Values written as a string of a syntax of their own.

/// Implements [`ToData`](crate::ToData) and [`FromData`](crate::FromData) for `$ty`, written as its
/// `Display` and read back by its `FromStr`; a string that does not parse is a
/// mismatch naming `$syntax`, as "a version".
#[macro_export]
macro_rules! textual {
    ($ty:ty, $syntax:literal) => {
        impl $crate::ToData for $ty {
            fn to_data(&self) -> $crate::Value<'static> {
                $crate::ToData::to_data(&self.to_string())
            }
        }

        impl $crate::FromData for $ty {
            fn from_data(value: &$crate::Value<'_>) -> Result<Self, $crate::Mismatch> {
                let text = value.as_str().ok_or($crate::Mismatch::new($crate::Expected::String))?;
                text.parse::<$ty>().map_err(|_| $crate::Mismatch::new($crate::Expected::Syntax($syntax)))
            }
        }
    };
}
