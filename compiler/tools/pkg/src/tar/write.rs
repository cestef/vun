//! Packing: one ustar entry per file, with a PAX `path` record before a path too long for the
//! header, and fixed metadata, so that identical trees give identical archives.

use std::io;

use vun_fs::Mode;

use super::{BLOCK, Field, Header, Kind, PAX_PATH, Tar};
use crate::Contents;

/// The name of the header carrying a PAX record, as tools write it.
const PAX_NAME: &str = "PaxHeader";

impl Tar {
    /// The archive of `contents`.
    ///
    /// # Errors
    /// Returns an error if a file cannot be read.
    pub fn pack(contents: &Contents<'_>) -> io::Result<Vec<u8>> {
        let mut out = Vec::new();
        for file in contents.files() {
            let file = file?;
            let name = if file.path.len() <= Field::NAME.len {
                file.path.as_str()
            } else {
                let record = Self::pax_record(PAX_PATH, &file.path);
                Self::entry(&mut out, Kind::Extended, PAX_NAME, Mode::Regular, record.as_bytes());
                // Readers take the path from the record; the header keeps what fits.
                let mut end = Field::NAME.len;
                while !file.path.is_char_boundary(end) {
                    end -= 1;
                }
                &file.path[..end]
            };
            Self::entry(&mut out, Kind::File, name, file.mode, &file.bytes);
        }
        Self::end(&mut out);
        Ok(out)
    }

    /// Appends the end of an archive: two zero blocks.
    pub(super) fn end(out: &mut Vec<u8>) {
        out.resize(out.len() + 2 * BLOCK, 0);
    }

    /// Appends a header and `data`, padded to a block.
    pub(super) fn entry(out: &mut Vec<u8>, kind: Kind, name: &str, mode: Mode, data: &[u8]) {
        out.extend_from_slice(&Header::new(kind, name, mode, data.len() as u64).0);
        out.extend_from_slice(data);
        let padding = usize::try_from(Self::padding(data.len() as u64)).expect("less than a block");
        out.resize(out.len() + padding, 0);
    }

    /// The bytes after `size` bytes of data that reach the next block.
    pub(super) fn padding(size: u64) -> u64 {
        (BLOCK as u64 - size % BLOCK as u64) % BLOCK as u64
    }

    /// A PAX record, `<length> <key>=<value>\n`, whose length counts its own digits.
    pub(super) fn pax_record(key: &str, value: &str) -> String {
        let body = format!(" {key}={value}\n");
        let mut length = body.len();
        while length != body.len() + length.to_string().len() {
            length = body.len() + length.to_string().len();
        }
        format!("{length}{body}")
    }
}
