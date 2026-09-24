//! A compact binary encoding for persisted data, such as query memos (decision 61): integers as
//! LEB128 varints, sequences prefixed by their length, fields in declaration order. There is no
//! self-description; a reader knows the shape it reads.
//!
//! A type encodes itself: interned values say how they persist (a symbol by its text), so decoding
//! re-interns them in the reading process. A record type implements both traits with [`record!`].

mod impls;
#[cfg(test)]
mod tests;

use std::fmt;

pub trait Encode {
    fn encode(&self, out: &mut Vec<u8>);

    /// The encoding of `self` alone.
    fn encoded(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.encode(&mut out);
        out
    }
}

pub trait Decode: Sized {
    /// Reads a value from the front of `input`, advancing it.
    ///
    /// # Errors
    /// Returns why the bytes are not a value of this type.
    fn decode(input: &mut &[u8]) -> Result<Self, Malformed>;

    /// Reads a value that is the whole of `bytes`.
    ///
    /// # Errors
    /// Returns why the bytes are not exactly one value of this type.
    fn decoded(mut bytes: &[u8]) -> Result<Self, Malformed> {
        let value = Self::decode(&mut bytes)?;
        if bytes.is_empty() { Ok(value) } else { Err(Malformed::Trailing) }
    }
}

/// Why bytes do not decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Malformed {
    /// The input ends inside a value.
    Truncated,
    /// A varint does not fit its integer type.
    Overflow,
    /// Text that is not UTF-8.
    Text,
    /// A tag that names no variant.
    Tag(u64),
    /// Bytes remain after the value.
    Trailing,
}

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => f.write_str("the input ends inside a value"),
            Self::Overflow => f.write_str("an integer does not fit its type"),
            Self::Text => f.write_str("text is not UTF-8"),
            Self::Tag(tag) => write!(f, "tag {tag} names no variant"),
            Self::Trailing => f.write_str("bytes remain after the value"),
        }
    }
}

impl std::error::Error for Malformed {}

/// The bytes a value is decoded from, consumed from the front.
pub trait Input<'a> {
    /// Takes `n` bytes from the front.
    ///
    /// # Errors
    /// Returns [`Malformed::Truncated`] if fewer remain.
    fn front(&mut self, n: usize) -> Result<&'a [u8], Malformed>;
}

impl<'a> Input<'a> for &'a [u8] {
    fn front(&mut self, n: usize) -> Result<&'a [u8], Malformed> {
        if self.len() < n {
            return Err(Malformed::Truncated);
        }
        let (taken, rest) = self.split_at(n);
        *self = rest;
        Ok(taken)
    }
}

/// Implements [`Encode`] and [`Decode`] for a struct, field by field in the order listed.
#[macro_export]
macro_rules! record {
    ($ty:ident { $($field:ident),* $(,)? }) => {
        impl $crate::Encode for $ty {
            fn encode(&self, out: &mut Vec<u8>) {
                $($crate::Encode::encode(&self.$field, out);)*
            }
        }

        impl $crate::Decode for $ty {
            fn decode(input: &mut &[u8]) -> Result<Self, $crate::Malformed> {
                Ok(Self { $($field: $crate::Decode::decode(input)?),* })
            }
        }
    };
}
