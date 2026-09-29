//! Typed values as JSON and back: `ToData` writes a value, `FromData` reads one, most by
//! `#[derive(ToData, FromData)]`. Reading names where a value failed to fit and what was expected
//! there (`Mismatch`); an optional member may be absent, and an absent option is left out.

mod impls;
mod mismatch;
#[cfg(test)]
mod tests;

pub use mismatch::{Expected, Mismatch, Step};

use crate::{Object, Value};

/// A value written as JSON.
pub trait ToData {
    fn to_data(&self) -> Value<'static>;

    /// Whether a member holding it is left out of its object: an absent option.
    fn omitted(&self) -> bool {
        false
    }
}

/// A value read from JSON.
pub trait FromData: Sized {
    /// # Errors
    /// Where `value` does not fit, and what was expected there.
    fn from_data(value: &Value<'_>) -> Result<Self, Mismatch>;

    /// The value of a member left out, when it may be.
    #[must_use]
    fn absent() -> Option<Self> {
        None
    }
}

impl Value<'_> {
    /// This value as a `T`.
    ///
    /// # Errors
    /// Where it does not fit a `T`.
    pub fn read<T: FromData>(&self) -> Result<T, Mismatch> {
        T::from_data(self)
    }
}

impl Object<'_> {
    /// The member `name` as a `T`, or `T`'s absent value when it is left out.
    ///
    /// # Errors
    /// Where the member does not fit a `T`, or that it is missing.
    pub fn read<T: FromData>(&self, name: &'static str) -> Result<T, Mismatch> {
        let read = match self.get(name) {
            Some(value) => T::from_data(value),
            None => T::absent().ok_or(Mismatch::new(Expected::Member)),
        };
        read.map_err(|m| m.within(Step::Member(name)))
    }

    /// Appends the member `name` holding `value`, unless it is omitted.
    pub fn write(&mut self, name: &'static str, value: &impl ToData) {
        if !value.omitted() {
            self.push(name, value.to_data());
        }
    }
}

impl Object<'static> {
    /// Appends the members of `value`, an object; any other value adds none.
    pub fn merge(&mut self, value: Value<'static>) {
        if let Value::Object(members) = value {
            self.members.extend(members);
        }
    }
}
