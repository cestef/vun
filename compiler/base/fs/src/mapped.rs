//! A file read as memory, without reading it: a file of shared memos is tens of megabytes of
//! which a run asks for a few kilobytes, so the file is mapped and the system reads the pages
//! that are touched.

// The one place the crate maps memory; the argument for it is on `Mapped::open`.
#![allow(unsafe_code)]

use std::io;
use std::path::Path;

/// The bytes of a file, mapped read-only where the system maps files and read otherwise (under
/// WASI).
pub struct Mapped(Bytes);

#[cfg(not(target_os = "wasi"))]
type Bytes = memmap2::Mmap;
#[cfg(target_os = "wasi")]
type Bytes = Vec<u8>;

impl Mapped {
    /// Maps the file at `path`.
    ///
    /// # Errors
    /// Returns an error if the file cannot be opened or mapped.
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = std::fs::File::open(path)?;
        #[cfg(not(target_os = "wasi"))]
        // SAFETY: mapping is unsound only if the file changes while it is mapped. The files
        // mapped are those of shared memos (`vun-driver`'s `queries`), which are written whole
        // beside their name and renamed into place (`Edit::put`), never written in place or
        // truncated, and are removed by unlinking, which leaves a mapped file readable. A file
        // changed by another program is the same fault as a corrupt one, which the loader
        // refuses only by the checks it makes of the bytes it reads, as rustc's reading of
        // metadata does.
        let bytes = unsafe { memmap2::Mmap::map(&file)? };
        #[cfg(target_os = "wasi")]
        let bytes = std::io::Read::bytes(file).collect::<io::Result<Vec<u8>>>()?;
        Ok(Self(bytes))
    }
}

impl AsRef<[u8]> for Mapped {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}
