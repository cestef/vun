//! Writing what an archive holds, whatever its format: regular files only, with their executable
//! bit, each at a path of plain components inside the directory, and no more than
//! [`Unpacking::ENTRIES`] entries holding [`Unpacking::BYTES`] bytes in all, so that a small
//! archive cannot fill the disk or the memory before its hash is checked.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::{error, fmt};

use vun_fs::{Edit, Mode, Modes};
use vun_hash::Snapshot;

pub(crate) struct Unpacking<'d> {
    dir: &'d Path,
    entries: usize,
    bytes: u64,
}

/// Why an entry of an archive is not written; carried as the source of an
/// [`io::ErrorKind::InvalidData`].
#[derive(Debug)]
pub(crate) enum Refused {
    /// A path that is not plain components, which could name a file outside the tree.
    Path(String),
    /// An entry that is neither a file nor a directory, such as a link.
    Entry(String),
    Entries,
    Bytes,
}

impl<'d> Unpacking<'d> {
    /// The most entries an archive may hold.
    pub(crate) const ENTRIES: usize = 1 << 16;
    /// The most bytes its files may hold in all.
    pub(crate) const BYTES: u64 = 512 << 20;
    /// The most bytes of an archive's stream, before it is unpacked: its files, and the headers
    /// and padding around them, which never double a tree of [`Self::ENTRIES`] entries.
    pub(crate) const STREAM: u64 = 2 * Self::BYTES;

    pub(crate) fn into(dir: &'d Path) -> Self {
        Self { dir, entries: 0, bytes: 0 }
    }

    /// Counts an entry of any kind.
    pub(crate) fn entry(&mut self) -> io::Result<()> {
        self.entries += 1;
        if self.entries > Self::ENTRIES {
            return Err(Refused::Entries.into());
        }
        Ok(())
    }

    /// Writes the file at `path`, `./` allowed before it, with the `size` bytes `data` holds.
    pub(crate) fn file(&mut self, path: &str, data: impl Read, size: u64, mode: Mode) -> io::Result<()> {
        let target = self.target(path)?;
        self.bytes = self.bytes.saturating_add(size);
        if self.bytes > Self::BYTES {
            return Err(Refused::Bytes.into());
        }
        let mut bytes = Vec::new();
        data.take(size).read_to_end(&mut bytes)?;
        if bytes.len() as u64 != size {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        target.put(&bytes)?;
        target.set_mode(mode)
    }

    /// Where the entry at `path` is written.
    fn target(&self, path: &str) -> io::Result<PathBuf> {
        let components = Snapshot::components(path.trim_start_matches("./"))
            .ok_or_else(|| Refused::Path(path.to_owned()))?;
        Ok(components.iter().fold(self.dir.to_path_buf(), |p, c| p.join(c)))
    }
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Path(path) => write!(f, "`{path}` is not a plain path inside the archive"),
            Self::Entry(path) => write!(f, "`{path}` is neither a file nor a directory"),
            Self::Entries => write!(f, "more than {} entries", Unpacking::ENTRIES),
            Self::Bytes => write!(f, "more than {} bytes unpacked", Unpacking::BYTES),
        }
    }
}

impl error::Error for Refused {}

/// A writer refusing more than a number of bytes, for a stream decompressed whole.
pub(crate) struct Bounded<W> {
    pub(crate) inner: W,
    left: u64,
}

impl<W> Bounded<W> {
    pub(crate) fn new(inner: W, most: u64) -> Self {
        Self { inner, left: most }
    }
}

impl<W: Write> Write for Bounded<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.left = self.left.checked_sub(buf.len() as u64).ok_or(Refused::Bytes)?;
        self.inner.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl From<Refused> for io::Error {
    fn from(reason: Refused) -> Self {
        Self::new(io::ErrorKind::InvalidData, reason)
    }
}
