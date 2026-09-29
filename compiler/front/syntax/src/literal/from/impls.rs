//! `FromLiteral` for the types values are made of.

use super::{Expected, Fields, FromLiteral, Mismatch, Name, Path, Step};
use crate::literal::Literal;

impl FromLiteral for String {
    fn from_literal(value: &Literal<'_>) -> Result<Self, Mismatch> {
        value.as_str().map(str::to_owned).ok_or(Mismatch::new(Expected::String))
    }
}

impl FromLiteral for Name {
    fn from_literal(value: &Literal<'_>) -> Result<Self, Mismatch> {
        value.as_name().map(|n| Self(n.to_owned())).ok_or(Mismatch::new(Expected::Name))
    }
}

impl FromLiteral for Path {
    fn from_literal(value: &Literal<'_>) -> Result<Self, Mismatch> {
        match value {
            Literal::Path(segments) => Ok(Self(segments.iter().map(|s| (*s).to_owned()).collect())),
            _ => Err(Mismatch::new(Expected::Path)),
        }
    }
}

macro_rules! integers {
    ($($ty:ty),*) => {$(
        impl FromLiteral for $ty {
            fn from_literal(value: &Literal<'_>) -> Result<Self, Mismatch> {
                match value {
                    Literal::Int(n) => Self::try_from(*n).map_err(|_| Mismatch::new(Expected::Integer)),
                    _ => Err(Mismatch::new(Expected::Integer)),
                }
            }
        }
    )*};
}

integers!(u8, u16, u32, u64, usize, i32, i64, i128);

impl<T: FromLiteral> FromLiteral for Vec<T> {
    fn from_literal(value: &Literal<'_>) -> Result<Self, Mismatch> {
        let items = value.as_list().ok_or(Mismatch::new(Expected::List))?;
        let read = items.iter().enumerate();
        read.map(|(i, item)| T::from_literal(item).map_err(|m| m.within(Step::Item(i)))).collect()
    }
}

impl<T: FromLiteral> FromLiteral for Option<T> {
    fn from_literal(value: &Literal<'_>) -> Result<Self, Mismatch> {
        T::from_literal(value).map(Some)
    }

    fn absent() -> Option<Self> {
        Some(None)
    }
}

impl<T: FromLiteral> FromLiteral for Box<T> {
    fn from_literal(value: &Literal<'_>) -> Result<Self, Mismatch> {
        T::from_literal(value).map(Box::new)
    }
}

impl<T: FromLiteral> FromLiteral for Fields<T> {
    fn from_literal(value: &Literal<'_>) -> Result<Self, Mismatch> {
        let decls = value.decls().ok_or(Mismatch::new(Expected::Record))?;
        let read = decls.iter().enumerate().map(|(i, d)| {
            T::from_literal(&d.value).map(|v| (d.name.to_owned(), v)).map_err(|m| m.within(Step::Entry(i)))
        });
        read.collect::<Result<_, _>>().map(Self)
    }
}
