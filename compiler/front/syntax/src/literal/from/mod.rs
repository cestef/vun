//! Typed values read from literals: `FromLiteral` reads one, most by `#[derive(FromLiteral)]`
//! (`vun-literal-derive`, whose doc says how an item's shape maps to a literal), naming where a
//! value failed to fit and what was expected there ([`Mismatch`]).

mod impls;
mod mismatch;

pub use mismatch::{Expected, Mismatch, Step};

use super::{Decl, Literal};

/// A value read from a literal.
pub trait FromLiteral: Sized {
    /// # Errors
    /// Where `value` does not fit, and what was expected there.
    fn from_literal(value: &Literal<'_>) -> Result<Self, Mismatch>;

    /// The value of a field left out, when it may be.
    #[must_use]
    fn absent() -> Option<Self> {
        None
    }

    /// The declarations of a file, read as the fields of one record.
    ///
    /// # Errors
    /// Where a declaration does not fit, or one needed is missing.
    fn from_decls(decls: &[Decl<'_>]) -> Result<Self, Mismatch> {
        Self::from_literal(&Literal::Record(decls.to_vec()))
    }
}

/// A `.name`, read as its name.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Name(pub String);

/// A path naming a declaration, `a.b`, read as its segments.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Path(pub Vec<String>);

/// The fields of a record whose names are data, not a type's: each with its value, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fields<T>(pub Vec<(String, T)>);

/// The payload of a tagged value `.tag(a, b)`, read by position.
pub struct Arguments<'v, 's> {
    values: &'v [Literal<'s>],
}

impl<'s> Literal<'s> {
    /// This value as a `T`.
    ///
    /// # Errors
    /// Where it does not fit a `T`.
    pub fn read<T: FromLiteral>(&self) -> Result<T, Mismatch> {
        T::from_literal(self)
    }

    /// The field `name` of a record as a `T`, or `T`'s absent value where it is left out.
    ///
    /// # Errors
    /// This is no record, the field does not fit a `T`, or it is missing.
    pub fn field<T: FromLiteral>(&self, name: &'static str) -> Result<T, Mismatch> {
        let decls = self.decls().ok_or(Mismatch::new(Expected::Record))?;
        let read = match decls.iter().find(|d| d.name == name) {
            Some(d) => T::from_literal(&d.value),
            None => T::absent().ok_or(Mismatch::new(Expected::Field)),
        };
        read.map_err(|m| m.within(Step::Field(name)))
    }

    /// Whether this is `.name`.
    #[must_use]
    pub fn is_name(&self, name: &str) -> bool {
        self.as_name() == Some(name)
    }

    /// The payload of this value if it is tagged `tag`, holding `count` values, or at least one
    /// fewer if the last reads the rest.
    ///
    /// # Errors
    /// It is tagged `tag` but holds another number of values.
    pub fn arguments(
        &self,
        tag: &str,
        count: usize,
        rest: bool,
    ) -> Result<Option<Arguments<'_, 's>>, Mismatch> {
        let Some((own, values)) = self.as_tagged() else { return Ok(None) };
        if own != tag {
            return Ok(None);
        }
        let fits = if rest { values.len() + 1 >= count } else { values.len() == count };
        if !fits {
            return Err(Mismatch::new(Expected::Arguments(count)));
        }
        Ok(Some(Arguments { values }))
    }
}

impl Arguments<'_, '_> {
    /// The value at `i` as a `T`.
    ///
    /// # Errors
    /// Where it does not fit a `T`.
    pub fn at<T: FromLiteral>(&self, i: usize) -> Result<T, Mismatch> {
        let value = self.values.get(i).ok_or(Mismatch::new(Expected::Arguments(i + 1)))?;
        T::from_literal(value).map_err(|m| m.within(Step::Argument(i)))
    }

    /// The values from `i` on, each a `T`.
    ///
    /// # Errors
    /// Where one does not fit a `T`.
    pub fn from<T: FromLiteral>(&self, i: usize) -> Result<Vec<T>, Mismatch> {
        let rest = self.values.iter().enumerate().skip(i);
        rest.map(|(at, v)| T::from_literal(v).map_err(|m| m.within(Step::Argument(at)))).collect()
    }
}
