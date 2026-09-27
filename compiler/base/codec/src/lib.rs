//! A compact binary encoding for persisted data, such as query memos (decision 61): integers as
//! LEB128 varints, sequences prefixed by their length, fields in declaration order, variants by
//! their position. There is no self-description; a reader knows the shape it reads.
//!
//! A type encodes itself, most by `#[derive(Encode, Decode)]`. Values persist by content: an
//! interned value by what it holds, re-interned when read. A handle into a process's own table,
//! such as a file or an origin, is [`Pooled`]: written once per stream into its pool and
//! referenced by number, then recorded again in the reading process's table when first read.

mod blob;
mod impls;
mod pool;
mod reader;
#[cfg(test)]
mod tests;
mod writer;

use std::fmt;

pub use blob::Blob;
pub use pool::{Pool, Pooled, Pools};
pub use reader::{Context, Reader};
pub use vun_codec_derive::{Decode, Encode};
pub use writer::Writer;

// The derives name this crate by its path, here as anywhere else.
extern crate self as vun_codec;

pub trait Encode {
    fn encode(&self, w: &mut Writer<'_>);

    /// The encoding of `self` alone, which holds no pooled handle.
    fn encoded(&self) -> Vec<u8> {
        let mut w = Writer::new();
        self.encode(&mut w);
        w.into_bytes()
    }
}

pub trait Decode: Sized {
    /// Reads a value from the front of `r`, advancing it.
    ///
    /// # Errors
    /// Returns why the bytes are not a value of this type.
    fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed>;

    /// Reads a value that is the whole of `bytes`, which hold no pooled handle.
    ///
    /// # Errors
    /// Returns why the bytes are not exactly one value of this type.
    fn decoded(bytes: &[u8]) -> Result<Self, Malformed> {
        Reader::plain(bytes)
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
    /// A pooled handle numbers no entry of its pool.
    Unpooled { pool: &'static str, number: usize },
    /// A value that decodes but breaks an invariant of its type.
    Invariant,
    /// A pooled entry that refers to itself.
    Cycle,
}

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => f.write_str("the input ends inside a value"),
            Self::Overflow => f.write_str("an integer does not fit its type"),
            Self::Text => f.write_str("text is not UTF-8"),
            Self::Tag(tag) => write!(f, "tag {tag} names no variant"),
            Self::Trailing => f.write_str("bytes remain after the value"),
            Self::Unpooled { pool, number } => write!(f, "{pool} {number} is not in the stream"),
            Self::Invariant => f.write_str("a value breaks its type's invariant"),
            Self::Cycle => f.write_str("a pooled entry refers to itself"),
        }
    }
}

impl std::error::Error for Malformed {}
