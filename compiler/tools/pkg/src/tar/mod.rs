//! Tar archives, as forges and vendors publish them: ustar headers, with long paths in GNU `L`
//! records or PAX `path` records. A package tree holds files only (spec 12, section 3), so only
//! regular files are written and kept, with their executable bit; directories are implied by the
//! files in them, and links are refused.

mod read;
#[cfg(test)]
mod tests;
mod write;

use std::io;

use vun_fs::Mode;
use vun_hash::Snapshot;

/// Records come in blocks of this size, and every header is one block.
const BLOCK: usize = 512;

/// A ustar header field, by offset and length in the block.
#[derive(Clone, Copy)]
struct Field {
    at: usize,
    len: usize,
}

impl Field {
    const NAME: Self = Self { at: 0, len: 100 };
    const MODE: Self = Self { at: 100, len: 8 };
    const SIZE: Self = Self { at: 124, len: 12 };
    const CHECKSUM: Self = Self { at: 148, len: 8 };
    const KIND: Self = Self { at: 156, len: 1 };
    const MAGIC: Self = Self { at: 257, len: 8 };
    const PREFIX: Self = Self { at: 345, len: 155 };

    fn range(self) -> std::ops::Range<usize> {
        self.at..self.at + self.len
    }
}

/// The magic and version of a POSIX ustar header.
const USTAR: &[u8; 8] = b"ustar\x0000";
/// The PAX record naming the entry that follows.
const PAX_PATH: &str = "path";

/// What an entry holds, by its header's kind byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    File,
    Directory,
    /// A GNU record whose data is the next entry's path.
    LongName,
    /// PAX records for the next entry.
    Extended,
    /// PAX records for every entry, which say nothing a package keeps.
    Global,
    /// Links, devices and the like.
    Other(u8),
}

impl Kind {
    /// The kinds by byte; `b'\0'` and `b'7'` are regular files too.
    const BYTES: [(u8, Self); 5] = [
        (b'0', Self::File),
        (b'5', Self::Directory),
        (b'L', Self::LongName),
        (b'x', Self::Extended),
        (b'g', Self::Global),
    ];

    fn of(byte: u8) -> Self {
        match byte {
            b'\0' | b'7' => Self::File,
            _ => Self::BYTES.iter().find(|(b, _)| *b == byte).map_or(Self::Other(byte), |&(_, k)| k),
        }
    }

    fn byte(self) -> u8 {
        match self {
            Self::Other(byte) => byte,
            kind => {
                Self::BYTES.iter().find(|(_, k)| *k == kind).map(|&(b, _)| b).expect("every kind has a byte")
            }
        }
    }
}

/// A ustar header block.
struct Header([u8; BLOCK]);

impl Header {
    /// The header of an entry of `kind` named `name` (at most [`Field::NAME`] bytes), with `mode`
    /// and `size`, checksummed.
    fn new(kind: Kind, name: &str, mode: Mode, size: u64) -> Self {
        let mut header = Self([0; BLOCK]);
        header.0[Field::NAME.at..Field::NAME.at + name.len()].copy_from_slice(name.as_bytes());
        header.put_number(Field::MODE, mode.bits().into());
        header.put_number(Field::SIZE, size);
        header.0[Field::KIND.at] = kind.byte();
        header.0[Field::MAGIC.range()].copy_from_slice(USTAR);
        header.put_number(Field::CHECKSUM, header.checksum());
        header
    }

    /// The sum of the bytes with the checksum field counted as spaces.
    fn checksum(&self) -> u64 {
        let spaces = u64::from(b' ') * Field::CHECKSUM.len as u64;
        let others: u64 = self
            .0
            .iter()
            .enumerate()
            .filter(|(i, _)| !Field::CHECKSUM.range().contains(i))
            .map(|(_, &b)| u64::from(b))
            .sum();
        others + spaces
    }

    /// Writes `n` in octal, zero-padded, NUL-terminated, as ustar does.
    fn put_number(&mut self, field: Field, n: u64) {
        let digits = format!("{n:0width$o}\0", width = field.len - 1);
        self.0[field.range()].copy_from_slice(digits.as_bytes());
    }

    fn number(&self, field: Field) -> io::Result<u64> {
        let text = Tar::text(&self.0[field.range()]).trim_matches(|c| c == ' ' || c == '\0');
        Ok(u64::from_str_radix(text, 8).map_err(|_| Malformed::Header)?)
    }

    /// The executable bit of the permissions.
    fn mode(&self) -> io::Result<Mode> {
        let bits = u32::try_from(self.number(Field::MODE)?).map_err(|_| Malformed::Header)?;
        Ok(Mode::of_bits(bits))
    }

    fn kind(&self) -> Kind {
        Kind::of(self.0[Field::KIND.at])
    }

    /// The ustar path: the prefix, then the name.
    fn path(&self) -> String {
        let name = Tar::text(&self.0[Field::NAME.range()]);
        match Tar::text(&self.0[Field::PREFIX.range()]) {
            "" => name.to_owned(),
            prefix => [prefix, name].join(&Snapshot::PATH_SEPARATOR.to_string()),
        }
    }
}

/// How a tar archive is malformed; carried as the source of an [`io::ErrorKind::InvalidData`].
#[derive(Debug)]
pub(crate) enum Malformed {
    /// A header whose checksum or numbers do not read.
    Header,
    /// A path that is not plain components, which could name a file outside the tree.
    Path(String),
    /// An entry that is not a file or a directory, by its kind byte.
    Unsupported(char),
}

impl std::fmt::Display for Malformed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Header => f.write_str("a malformed tar header"),
            Self::Path(path) => write!(f, "`{path}` is not a plain path inside the archive"),
            Self::Unsupported(kind) => write!(f, "an entry of kind `{kind}`, which a package cannot hold"),
        }
    }
}

impl std::error::Error for Malformed {}

impl From<Malformed> for io::Error {
    fn from(reason: Malformed) -> Self {
        Self::new(io::ErrorKind::InvalidData, reason)
    }
}

pub struct Tar;

impl Tar {
    /// A NUL-terminated field or record.
    fn text(bytes: &[u8]) -> &str {
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        std::str::from_utf8(&bytes[..end]).unwrap_or_default()
    }
}
