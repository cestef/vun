//! Reading what `Out` writes: little-endian fixed widths and zero-ended strings, every read
//! checked against the end of the bytes.

use std::fmt;

/// A read that ends past the bytes: where it ends, and where the bytes do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Truncated {
    pub at: u64,
    pub end: u64,
}

impl fmt::Display for Truncated {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the bytes end at {}, before {}", self.end, self.at)
    }
}

impl std::error::Error for Truncated {}

/// Bytes read from an offset on.
#[derive(Clone, Copy, Debug)]
pub struct In<'a> {
    bytes: &'a [u8],
    at: u64,
}

impl<'a> In<'a> {
    /// `bytes`, read from `at` on.
    #[must_use]
    pub const fn at(bytes: &'a [u8], at: u64) -> Self {
        Self { bytes, at }
    }

    /// Where the next read starts.
    #[must_use]
    pub const fn position(&self) -> u64 {
        self.at
    }

    /// Moves the next read to `at`.
    pub fn seek(&mut self, at: u64) {
        self.at = at;
    }

    /// The next `n` bytes.
    ///
    /// # Errors
    /// Where they end past the bytes.
    pub fn take(&mut self, n: u64) -> Result<&'a [u8], Truncated> {
        let end = self.bytes.len() as u64;
        let truncated = Truncated { at: self.at.saturating_add(n), end };
        let range = self
            .at
            .checked_add(n)
            .and_then(|to| Some((usize::try_from(self.at).ok()?, usize::try_from(to).ok()?)));
        let taken = range.and_then(|(from, to)| self.bytes.get(from..to)).ok_or(truncated)?;
        self.at += n;
        Ok(taken)
    }

    /// The next `N` bytes, as an array.
    ///
    /// # Errors
    /// Where they end past the bytes.
    ///
    /// # Panics
    /// Never: the bytes taken are `N`.
    pub fn array<const N: usize>(&mut self) -> Result<[u8; N], Truncated> {
        Ok(self.take(N as u64)?.try_into().expect("N bytes taken"))
    }

    /// The next byte.
    ///
    /// # Errors
    /// Where the bytes end.
    pub fn u8(&mut self) -> Result<u8, Truncated> {
        Ok(self.array::<1>()?[0])
    }

    /// The next 16 bits, little-endian.
    ///
    /// # Errors
    /// Where they end past the bytes.
    pub fn u16(&mut self) -> Result<u16, Truncated> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    /// The next 32 bits, little-endian.
    ///
    /// # Errors
    /// Where they end past the bytes.
    pub fn u32(&mut self) -> Result<u32, Truncated> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    /// The next 64 bits, little-endian.
    ///
    /// # Errors
    /// Where they end past the bytes.
    pub fn u64(&mut self) -> Result<u64, Truncated> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    /// The zero-ended string at `at` of `bytes`, without its zero.
    ///
    /// # Errors
    /// Where `at` is past the bytes or no zero ends the string.
    pub fn string(bytes: &'a [u8], at: u64) -> Result<&'a [u8], Truncated> {
        let end = bytes.len() as u64;
        let rest = usize::try_from(at).ok().and_then(|at| bytes.get(at..)).ok_or(Truncated { at, end })?;
        let length = rest.iter().position(|&b| b == 0).ok_or(Truncated { at: end + 1, end })?;
        Ok(&rest[..length])
    }
}
