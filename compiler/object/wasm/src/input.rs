//! Reading a module's bytes a field at a time, every read checked against the end.

use std::fmt;

use vun_bytes::PADDED;

use crate::format::{Limits, ValType};

/// Why bytes are not a relocatable object the linker reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Malformed {
    /// Something ends past the end of what holds it, which is at `end`.
    Truncated { at: usize, end: usize },
    /// The first bytes are not a wasm module's, or of a version other than 1.
    Magic,
    /// A LEB128 longer than its type allows.
    Leb { at: usize },
    /// A name that is not UTF-8.
    Name { at: usize },
    /// A code that names nothing where it is read: a section, a value type, an import kind,
    /// a symbol kind, a relocation kind, a subsection of `linking`.
    Code { what: Coded, code: u32, at: usize },
    /// Something the linker does not read: a section of this id in an object, a data segment
    /// that is not active in memory 0, a symbol of this kind a relocation names.
    Unsupported { what: Coded, code: u32 },
    /// An index past the end of what it indexes.
    Index { what: Coded, index: u32 },
    /// An object with no `linking` section, or one of another version.
    Linking,
}

/// What a code or an index names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coded {
    Section,
    ValType,
    Import,
    Symbol,
    Reloc,
    Subsection,
    Segment,
    Type,
    Function,
    Constant,
    Limits,
    Comdat,
}

impl fmt::Display for Coded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Section => "section",
            Self::ValType => "value type",
            Self::Import => "import kind",
            Self::Symbol => "symbol",
            Self::Reloc => "relocation",
            Self::Subsection => "subsection of `linking`",
            Self::Segment => "data segment",
            Self::Type => "type",
            Self::Function => "function",
            Self::Constant => "constant expression",
            Self::Limits => "limits",
            Self::Comdat => "COMDAT member",
        })
    }
}

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { at, end } => write!(f, "it ends at byte {end}, before byte {at}"),
            Self::Magic => f.write_str("it does not start as a wasm module of version 1 does"),
            Self::Leb { at } => write!(f, "the LEB128 at byte {at} is too long"),
            Self::Name { at } => write!(f, "the name at byte {at} is not UTF-8"),
            Self::Code { what, code, at } => write!(f, "{code} at byte {at} names no {what}"),
            Self::Unsupported { what, code } => {
                write!(f, "it has a {what} of kind {code}, which the linker does not read")
            }
            Self::Index { what, index } => write!(f, "{what} {index} is out of range"),
            Self::Linking => f.write_str("it has no `linking` section of version 2: it is not relocatable"),
        }
    }
}

impl std::error::Error for Malformed {}

/// Bytes read from an offset on.
#[derive(Clone, Copy)]
pub(crate) struct Input<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) at: usize,
}

impl<'a> Input<'a> {
    pub(crate) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    pub(crate) fn done(&self) -> bool {
        self.at >= self.bytes.len()
    }

    /// The next `n` bytes.
    pub(crate) fn take(&mut self, n: usize) -> Result<&'a [u8], Malformed> {
        let end = self.at.checked_add(n).filter(|&end| end <= self.bytes.len());
        let end = end.ok_or(Malformed::Truncated { at: self.at.saturating_add(n), end: self.bytes.len() })?;
        let taken = &self.bytes[self.at..end];
        self.at = end;
        Ok(taken)
    }

    pub(crate) fn u8(&mut self) -> Result<u8, Malformed> {
        Ok(self.take(1)?[0])
    }

    pub(crate) fn u32(&mut self) -> Result<u32, Malformed> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().expect("four bytes")))
    }

    pub(crate) fn uleb(&mut self) -> Result<u32, Malformed> {
        let start = self.at;
        let (value, _) = self.leb()?;
        u32::try_from(value).map_err(|_| Malformed::Leb { at: start })
    }

    pub(crate) fn sleb(&mut self) -> Result<i32, Malformed> {
        let start = self.at;
        let (value, bits) = self.leb()?;
        // Sign-extended from the last byte's sign bit.
        let signed =
            if bits < 64 && value >> (bits - 1) & 1 == 1 { value | (u64::MAX << bits) } else { value };
        #[expect(clippy::cast_possible_wrap, reason = "two's complement bits")]
        i32::try_from(signed as i64).map_err(|_| Malformed::Leb { at: start })
    }

    /// The bits of a LEB128 of at most [`PADDED`] bytes, and how many it has.
    fn leb(&mut self) -> Result<(u64, u32), Malformed> {
        let start = self.at;
        let mut value: u64 = 0;
        for i in 0..PADDED {
            let byte = self.u8()?;
            let shift = 7 * u32::try_from(i).expect("five");
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok((value, shift + 7));
            }
        }
        Err(Malformed::Leb { at: start })
    }

    /// A length, then as many bytes.
    pub(crate) fn sized(&mut self) -> Result<&'a [u8], Malformed> {
        let n = self.uleb()?;
        self.take(n as usize)
    }

    pub(crate) fn name(&mut self) -> Result<&'a str, Malformed> {
        let at = self.at;
        std::str::from_utf8(self.sized()?).map_err(|_| Malformed::Name { at })
    }

    /// A count, then as many items as `item` reads.
    pub(crate) fn vector<T>(
        &mut self,
        mut item: impl FnMut(&mut Self) -> Result<T, Malformed>,
    ) -> Result<Vec<T>, Malformed> {
        let n = self.uleb()?;
        (0..n).map(|_| item(self)).collect()
    }

    /// A code `of` reads, or why it names no `what`.
    pub(crate) fn coded<T>(&mut self, what: Coded, of: impl FnOnce(u8) -> Option<T>) -> Result<T, Malformed> {
        let at = self.at;
        let code = self.u8()?;
        of(code).ok_or(Malformed::Code { what, code: u32::from(code), at })
    }

    pub(crate) fn val_type(&mut self) -> Result<ValType, Malformed> {
        self.coded(Coded::ValType, ValType::of)
    }

    pub(crate) fn limits(&mut self) -> Result<Limits, Malformed> {
        let at = self.at;
        match self.u8()? {
            0 => Ok(Limits { min: self.uleb()?, max: None }),
            1 => Ok(Limits { min: self.uleb()?, max: Some(self.uleb()?) }),
            code => Err(Malformed::Code { what: Coded::Limits, code: u32::from(code), at }),
        }
    }
}
