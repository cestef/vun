//! Zip archives: read through the `zip` crate, each entry written through [`Unpacking`], and
//! written with fixed timestamps and the permissions of each file's mode, so that identical trees
//! give identical archives.

use std::io::{self, Cursor, Write};
use std::path::Path;

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter};

use vun_fs::Mode;

use crate::Contents;
use crate::unpack::{Refused, Unpacking};

pub struct Zip;

impl Zip {
    /// Unpacks `bytes` into `dir`.
    ///
    /// # Errors
    /// Returns an error if the archive is malformed or writing fails.
    pub fn unpack(bytes: &[u8], dir: &Path) -> io::Result<()> {
        let mut unpacking = Unpacking::into(dir);
        let mut zip = ZipArchive::new(Cursor::new(bytes)).map_err(io::Error::other)?;
        for index in 0..zip.len() {
            unpacking.entry()?;
            let entry = zip.by_index(index).map_err(io::Error::other)?;
            if entry.is_dir() {
                continue;
            }
            let path = entry.name().to_owned();
            if !entry.is_file() {
                return Err(Refused::Entry(path).into());
            }
            let mode = entry.unix_mode().map_or(Mode::Regular, Mode::of_bits);
            let size = entry.size();
            unpacking.file(&path, entry, size, mode)?;
        }
        Ok(())
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
            zip.start_file(file.path, options.unix_permissions(file.mode.bits()))
                .map_err(io::Error::other)?;
            zip.write_all(file.bytes)?;
        }
        Ok(zip.finish().map_err(io::Error::other)?.into_inner())
    }
}
