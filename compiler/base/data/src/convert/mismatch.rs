//! Why a value does not fit the type it is read as: what was expected, and where.

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
    Member(&'static str),
    Item(usize),
}

/// What a value was expected to be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expected {
    Null,
    Bool,
    Number,
    /// An integer number in the range of the type read.
    Integer,
    String,
    Array,
    /// An array of this many items.
    Items(usize),
    Object,
    /// A member present, as it is not optional.
    Member,
    /// One of these strings, or numbers written as such.
    OneOf(&'static [&'static str]),
    /// A value of one of the forms of a type that is several.
    Form,
    /// A string written in a syntax of its own, which this names: "a URI".
    Syntax(&'static str),
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
            Self::Member(name) => write!(f, ".{name}"),
            Self::Item(i) => write!(f, "[{i}]"),
        }
    }
}

impl fmt::Display for Expected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => f.write_str("null"),
            Self::Bool => f.write_str("a boolean"),
            Self::Number => f.write_str("a number"),
            Self::Integer => f.write_str("an integer in range"),
            Self::String => f.write_str("a string"),
            Self::Array => f.write_str("an array"),
            Self::Items(n) => write!(f, "an array of {n} items"),
            Self::Object => f.write_str("an object"),
            Self::Member => f.write_str("the member, which is not optional"),
            Self::OneOf(names) => {
                f.write_str("one of ")?;
                for (i, name) in names.iter().enumerate() {
                    let separator = if i == 0 { "" } else { ", " };
                    write!(f, "{separator}`{name}`")?;
                }
                Ok(())
            }
            Self::Form => f.write_str("a value of one of its forms"),
            Self::Syntax(what) => f.write_str(what),
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
