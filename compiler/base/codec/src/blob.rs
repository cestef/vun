//! Bytes read in place.

use crate::{Decode, Malformed, Reader};

/// A byte string read as where it is in the input rather than copied: written as the bytes of a
/// `Vec<u8>` or `[u8]` (their length, then themselves), read as their place.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Blob {
    /// Its offset in the input.
    pub at: usize,
    pub len: usize,
}

impl Blob {
    /// Its bytes in `input`, the input it was read from.
    ///
    /// # Panics
    /// If it lies outside `input`.
    #[must_use]
    pub fn of(self, input: &[u8]) -> &[u8] {
        &input[self.at..self.at + self.len]
    }

    /// Reads a value that is the whole of these bytes of `input`, holding no pooled handle, so that
    /// the places it holds are places in `input`.
    ///
    /// # Errors
    /// Returns why the bytes are not exactly one value of this type.
    pub fn read<T: Decode>(self, input: &[u8]) -> Result<T, Malformed> {
        Reader::plain_at(self.of(input), self.at)
    }
}

impl Decode for Blob {
    fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        let len = usize::decode(r)?;
        let at = r.offset();
        r.take(len)?;
        Ok(Self { at, len })
    }
}
