//! Unpacking: regular files are written through [`Unpacking`], which checks their paths and
//! bounds them.

use std::io::{self, Read};
use std::path::Path;

use super::{BLOCK, Field, Header, Kind, Malformed, PAX_PATH, Tar};
use crate::unpack::Unpacking;

/// The most bytes a record naming the next entry holds, far past any path.
const RECORD: u64 = 1 << 20;

impl Tar {
    /// Unpacks the archive read from `input` into `dir`.
    ///
    /// # Errors
    /// Returns an error if the archive is malformed or holds a link, or writing fails.
    pub fn unpack(mut input: impl Read, dir: &Path) -> io::Result<()> {
        let mut unpacking = Unpacking::into(dir);
        // A path from a GNU or PAX record, for the entry that follows.
        let mut named: Option<String> = None;
        while let Some(header) = Self::header(&mut input)? {
            unpacking.entry()?;
            if header.number(Field::CHECKSUM)? != header.checksum() {
                return Err(Malformed::Header.into());
            }
            let size = header.number(Field::SIZE)?;
            match header.kind() {
                Kind::LongName => named = Some(Self::text(&Self::record(&mut input, size)?).to_owned()),
                Kind::Extended => named = Self::pax_path(&Self::record(&mut input, size)?).or(named),
                Kind::Global => drop(Self::record(&mut input, size)?),
                // A directory is implied by its files, but a long path before it is its own.
                Kind::Directory => {
                    named = None;
                    Self::skip(&mut input, size + Self::padding(size))?;
                }
                Kind::Other(byte) => return Err(Malformed::Unsupported(char::from(byte)).into()),
                Kind::File => {
                    let path = named.take().unwrap_or_else(|| header.path());
                    unpacking.file(&path, &mut input, size, header.mode()?)?;
                    Self::skip(&mut input, Self::padding(size))?;
                }
            }
        }
        Ok(())
    }

    /// The next header, or `None` at the end: the input's, or the zero block that marks it.
    fn header(input: &mut impl Read) -> io::Result<Option<Header>> {
        let mut block = [0; BLOCK];
        let mut filled = 0;
        while filled < BLOCK {
            match input.read(&mut block[filled..])? {
                0 if filled == 0 => return Ok(None),
                0 => return Err(io::ErrorKind::UnexpectedEof.into()),
                n => filled += n,
            }
        }
        Ok(block.iter().any(|&b| b != 0).then_some(Header(block)))
    }

    /// The `size` bytes of a record naming the next entry, then the padding to the next block.
    fn record(input: &mut impl Read, size: u64) -> io::Result<Vec<u8>> {
        if size > RECORD {
            return Err(Malformed::Header.into());
        }
        let mut data = Vec::new();
        input.take(size).read_to_end(&mut data)?;
        if data.len() as u64 != size {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        Self::skip(input, Self::padding(size))?;
        Ok(data)
    }

    /// Passes over `size` bytes.
    fn skip(input: &mut impl Read, size: u64) -> io::Result<()> {
        if io::copy(&mut input.take(size), &mut io::sink())? != size {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        Ok(())
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
}
