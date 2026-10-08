//! How the values a root gives are folded into the hash `main` prints: each scalar of a value,
//! in order, is mixed in, and each variant of an enum by its number first, so that what prints
//! says what every part was.

use crate::float::Format;
use crate::int::{BigInt, Range};
use crate::ty::{Nominal, Ty};

/// A step into a part of an aggregate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// A tuple's item, written `.0`.
    Field(usize),
    /// A struct's field, written by its name.
    Member(usize),
    Item(usize),
}

/// What of a scalar goes into the hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Piece {
    /// The low 64 bits of an integer.
    Low,
    /// The 64 bits above those, of an integer of more bits.
    High,
}

/// Bits of the hash, and of each piece of an integer.
pub const WORD: u32 = 64;

/// What of a value of one type is mixed in, and in what order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fold {
    /// An integer of the range: its pieces.
    Int(Range),
    Bool,
    Float(Format),
    /// The parts of an aggregate, each by the step that reaches it.
    Parts(Vec<(Step, Self)>),
    /// The variant's number, then what its payload holds, for each variant of an enum.
    Variants(Nominal, Vec<Vec<Self>>),
}

impl Ty {
    /// What of a value of this type is folded into the hash.
    #[must_use]
    pub fn fold(&self) -> Fold {
        match self {
            Self::Int(range) => Fold::Int(range.clone()),
            Self::Bool => Fold::Bool,
            Self::Float(format) => Fold::Float(*format),
            Self::Unit | Self::Ref(..) | Self::Closure(..) | Self::Param(_) => Fold::Parts(Vec::new()),
            Self::Tuple(parts) => {
                Fold::Parts(parts.iter().enumerate().map(|(i, p)| (Step::Field(i), p.fold())).collect())
            }
            Self::Struct(_, parts) => {
                Fold::Parts(parts.iter().enumerate().map(|(i, p)| (Step::Member(i), p.fold())).collect())
            }
            Self::Array(len, item) => Fold::Parts((0..*len).map(|i| (Step::Item(i), item.fold())).collect()),
            Self::Enum(nominal, variants) => Fold::Variants(
                *nominal,
                variants.iter().map(|payload| payload.iter().map(Self::fold).collect()).collect(),
            ),
        }
    }
}

impl Fold {
    /// The pieces an integer of `range` is mixed in as.
    #[must_use]
    pub fn pieces(range: &Range) -> &'static [Piece] {
        if range.needs() > WORD { &[Piece::Low, Piece::High] } else { &[Piece::Low] }
    }
}

impl Piece {
    /// The integer `2^WORD`, which the high piece divides by.
    #[must_use]
    pub fn word_size() -> BigInt {
        BigInt::from(1) << WORD
    }
}
