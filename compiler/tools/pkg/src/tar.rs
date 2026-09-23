//! Reading tar archives, as forges and vendors publish them: ustar headers, with long paths from
//! GNU `L` records or PAX `path` records. Only regular files are written, with their executable
//! bit; directories are implied by the files in them. Links are refused, since a package tree
//! holds files only (spec 12, section 3), and every path must be plain components.

use std::io::{self, Read};
use std::path::Path;

use overt_fs::{Edit, Mode, Modes};
use overt_hash::Snapshot;

/// Records are read in blocks of this size, and every header is one block.
const BLOCK: usize = 512;

/// Where a ustar header keeps each field, as `(offset, length)`.
const NAME: (usize, usize) = (0, 100);
const MODE: (usize, usize) = (100, 8);
const SIZE: (usize, usize) = (124, 12);
const CHECKSUM: (usize, usize) = (148, 8);
const KIND: usize = 156;
const PREFIX: (usize, usize) = (345, 155);
/// Any mode bit that makes a file executable.
const EXECUTABLE: u64 = 0o111;
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
    fn of(byte: u8) -> Self {
        match byte {
            b'0' | b'\0' | b'7' => Self::File,
            b'5' => Self::Directory,
            b'L' => Self::LongName,
            b'x' => Self::Extended,
            b'g' => Self::Global,
            other => Self::Other(other),
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

pub struct Tar;

impl Tar {
    /// Unpacks the archive read from `input` into `dir`.
    ///
    /// # Errors
    /// Returns an error if the archive is malformed or holds a link, or writing fails.
    pub fn unpack(mut input: impl Read, dir: &Path) -> io::Result<()> {
        let mut path_override: Option<String> = None;
        loop {
            let mut header = [0; BLOCK];
            if !Self::block(&mut input, &mut header)? || header.iter().all(|&b| b == 0) {
                return Ok(());
            }
            let sum: u64 = header
                .iter()
                .enumerate()
                .map(|(i, &b)| {
                    let inside = (CHECKSUM.0..CHECKSUM.0 + CHECKSUM.1).contains(&i);
                    u64::from(if inside { b' ' } else { b })
                })
                .sum();
            if Self::number(&header, CHECKSUM)? != sum {
                return Err(Self::invalid(Malformed::Header));
            }
            let size = Self::number(&header, SIZE)?;
            let data = Self::data(&mut input, size)?;
            match Kind::of(header[KIND]) {
                Kind::LongName => path_override = Some(Self::text(&data).to_owned()),
                Kind::Extended => path_override = Self::pax_path(&data).or(path_override),
                Kind::Global => {}
                // A directory is implied by its files, but its long path is its own.
                Kind::Directory => path_override = None,
                Kind::Other(byte) => return Err(Self::invalid(Malformed::Unsupported(char::from(byte)))),
                Kind::File => {
                    let path = path_override.take().unwrap_or_else(|| Self::header_path(&header));
                    let trimmed = path.trim_start_matches("./");
                    let components = Snapshot::components(trimmed)
                        .ok_or_else(|| Self::invalid(Malformed::Path(path.clone())))?;
                    let target = components.iter().fold(dir.to_path_buf(), |p, c| p.join(c));
                    target.put(&data)?;
                    let executable = Self::number(&header, MODE)? & EXECUTABLE != 0;
                    target.set_mode(if executable { Mode::Executable } else { Mode::Regular })?;
                }
            }
        }
    }

    /// Fills `block`; `false` at the end of the input before any byte of it.
    fn block(input: &mut impl Read, block: &mut [u8; BLOCK]) -> io::Result<bool> {
        let mut filled = 0;
        while filled < BLOCK {
            match input.read(&mut block[filled..])? {
                0 if filled == 0 => return Ok(false),
                0 => return Err(io::ErrorKind::UnexpectedEof.into()),
                n => filled += n,
            }
        }
        Ok(true)
    }

    /// The `size` bytes of an entry, then the padding to the next block.
    fn data(input: &mut impl Read, size: u64) -> io::Result<Vec<u8>> {
        let mut data = Vec::new();
        input.take(size).read_to_end(&mut data)?;
        if data.len() as u64 != size {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        let padding = (BLOCK as u64 - size % BLOCK as u64) % BLOCK as u64;
        io::copy(&mut input.take(padding), &mut io::sink())?;
        Ok(data)
    }

    /// A NUL-terminated field.
    fn text(bytes: &[u8]) -> &str {
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        std::str::from_utf8(&bytes[..end]).unwrap_or_default()
    }

    /// An octal number field, as ustar writes numbers.
    fn number(header: &[u8; BLOCK], (at, len): (usize, usize)) -> io::Result<u64> {
        let text = Self::text(&header[at..at + len]).trim_matches(|c| c == ' ' || c == '\0');
        u64::from_str_radix(text, 8).map_err(|_| Self::invalid(Malformed::Header))
    }

    /// The ustar path: the prefix, then the name.
    fn header_path(header: &[u8; BLOCK]) -> String {
        let name = Self::text(&header[NAME.0..NAME.0 + NAME.1]);
        match Self::text(&header[PREFIX.0..PREFIX.0 + PREFIX.1]) {
            "" => name.to_owned(),
            prefix => [prefix, name].join(&Snapshot::PATH_SEPARATOR.to_string()),
        }
    }

    /// The `path` among PAX records `<length> <key>=<value>\n`.
    fn pax_path(data: &[u8]) -> Option<String> {
        let text = std::str::from_utf8(data).ok()?;
        text.lines().find_map(|record| {
            let (_, entry) = record.split_once(' ')?;
            let (key, value) = entry.split_once('=')?;
            (key == PAX_PATH).then(|| value.to_owned())
        })
    }

    fn invalid(reason: Malformed) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, reason)
    }
}
