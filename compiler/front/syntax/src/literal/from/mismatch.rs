//! Why a literal does not fit the type it is read as: what was expected, and where.

use std::fmt;

/// A value that does not fit: what was expected, at the path from the value read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mismatch {
    pub expected: Expected,
    /// The steps to where it failed, innermost first.
    steps: Vec<Step>,
}

/// One step into a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// A field of a record, by its name as the type reads it.
    Field(&'static str),
    /// A field of a record read as data, by its position.
    Entry(usize),
    Item(usize),
    /// A value of a tagged value's payload.
    Argument(usize),
}

/// What a value was expected to be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expected {
    Name,
    String,
    /// An integer in the range of the type read.
    Integer,
    List,
    Record,
    Path,
    /// A field present, as it is not optional.
    Field,
    /// A payload of this many values.
    Arguments(usize),
    /// `.name` or `.name(...)` for one of these names.
    OneOf(&'static [&'static str]),
    /// A value of one of the forms of a type that is several.
    Form,
}

impl Mismatch {
    #[must_use]
    pub const fn new(expected: Expected) -> Self {
        Self { expected, steps: Vec::new() }
    }

    /// This mismatch, found one `step` into the value read.
    #[must_use]
    pub fn within(mut self, step: Step) -> Self {
        self.steps.push(step);
        self
    }

    /// The steps from the value read to where it failed, outermost first.
    pub fn path(&self) -> impl Iterator<Item = Step> {
        self.steps.iter().rev().copied()
    }
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Field(name) => write!(f, ".{name}"),
            Self::Entry(i) => write!(f, ".<{i}>"),
            Self::Item(i) => write!(f, "[{i}]"),
            Self::Argument(i) => write!(f, "({i})"),
        }
    }
}

impl fmt::Display for Expected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name => f.write_str("a `.name`"),
            Self::String => f.write_str("a string"),
            Self::Integer => f.write_str("an integer in range"),
            Self::List => f.write_str("a list"),
            Self::Record => f.write_str("a record"),
            Self::Path => f.write_str("a path"),
            Self::Field => f.write_str("the field, which is not optional"),
            Self::Arguments(n) => write!(f, "a payload of {n} values"),
            Self::OneOf(names) => {
                f.write_str("one of ")?;
                for (i, name) in names.iter().enumerate() {
                    let separator = if i == 0 { "" } else { ", " };
                    write!(f, "{separator}`.{name}`")?;
                }
                Ok(())
            }
            Self::Form => f.write_str("a value of one of its forms"),
        }
    }
}

impl fmt::Display for Mismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "expected {}", self.expected)?;
        if !self.steps.is_empty() {
            f.write_str(" at `")?;
            for step in self.path() {
                write!(f, "{step}")?;
            }
            f.write_str("`")?;
        }
        Ok(())
    }
}

impl std::error::Error for Mismatch {}
