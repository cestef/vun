//! Unpacking: regular files are written with their executable bit, every path checked to be
//! plain components.

use std::io::{self, Read};
use std::path::Path;

use overt_fs::{Edit, Modes};
use overt_hash::Snapshot;

use super::{BLOCK, Field, Header, Kind, Malformed, PAX_PATH, Tar};

impl Tar {
    /// Unpacks the archive read from `input` into `dir`.
    ///
    /// # Errors
    /// Returns an error if the archive is malformed or holds a link, or writing fails.
    pub fn unpack(mut input: impl Read, dir: &Path) -> io::Result<()> {
        // A path from a GNU or PAX record, for the entry that follows.
        let mut named: Option<String> = None;
        while let Some(header) = Self::header(&mut input)? {
            if header.number(Field::CHECKSUM)? != header.checksum() {
                return Err(Malformed::Header.into());
            }
            let data = Self::data(&mut input, header.number(Field::SIZE)?)?;
            match header.kind() {
                Kind::LongName => named = Some(Self::text(&data).to_owned()),
                Kind::Extended => named = Self::pax_path(&data).or(named),
                Kind::Global => {}
                // A directory is implied by its files, but a long path before it is its own.
                Kind::Directory => named = None,
                Kind::Other(byte) => return Err(Malformed::Unsupported(char::from(byte)).into()),
                Kind::File => {
                    let path = named.take().unwrap_or_else(|| header.path());
                    let components = Snapshot::components(path.trim_start_matches("./"))
                        .ok_or_else(|| Malformed::Path(path.clone()))?;
                    let target = components.iter().fold(dir.to_path_buf(), |p, c| p.join(c));
                    target.put(&data)?;
                    target.set_mode(header.mode()?)?;
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

    /// The `size` bytes of an entry, then the padding to the next block.
    fn data(input: &mut impl Read, size: u64) -> io::Result<Vec<u8>> {
        let mut data = Vec::new();
        input.take(size).read_to_end(&mut data)?;
        if data.len() as u64 != size {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        io::copy(&mut input.take(Self::padding(size)), &mut io::sink())?;
        Ok(data)
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
