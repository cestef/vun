//! Zip archives: read through the `zip` crate, which refuses paths that leave the directory, and
//! written with fixed timestamps and the permissions of each file's mode, so that identical trees
//! give identical archives.

use std::io::{self, Cursor, Write};
use std::path::Path;

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter};

use crate::Contents;

pub struct Zip;

impl Zip {
    /// Unpacks `bytes` into `dir`.
    ///
    /// # Errors
    /// Returns an error if the archive is malformed or writing fails.
    pub fn unpack(bytes: &[u8], dir: &Path) -> io::Result<()> {
        ZipArchive::new(Cursor::new(bytes)).and_then(|mut z| z.extract(dir)).map_err(io::Error::other)
    }

    /// The archive of `contents`.
    ///
    /// # Errors
    /// Returns an error if a file cannot be read.
    pub fn pack(contents: &Contents<'_>) -> io::Result<Vec<u8>> {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .last_modified_time(DateTime::default());
        for file in contents.files() {
            let file = file?;
            zip.start_file(file.path, options.unix_permissions(file.mode.bits()))
                .map_err(io::Error::other)?;
            zip.write_all(&file.bytes)?;
        }
        Ok(zip.finish().map_err(io::Error::other)?.into_inner())
    }
}
