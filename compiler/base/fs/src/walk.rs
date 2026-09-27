//! Directory walks in a stable order.

use std::io;
use std::path::{Path, PathBuf};

use crate::Refusal;

/// The directory the toolchain keeps its own files in, beside sources: builds, saved queries.
/// Never source, so walks leave it out, and nothing it holds changes what a package is.
pub const WORK: &str = ".vun";

/// Directory walks on paths.
pub trait Tree {
    /// The files at or below this path, recursively, sorted; a file is its own only entry.
    /// Symbolic links are not followed: a walk meeting one fails with [`Refusal::Link`].
    ///
    /// # Errors
    /// Returns an error if the path does not exist, a directory cannot be read or a link is met.
    fn files(&self) -> io::Result<Vec<PathBuf>>;

    /// The files at or below this path whose extension is `ext`, sorted.
    ///
    /// # Errors
    /// As [`Tree::files`].
    fn files_with(&self, ext: &str) -> io::Result<Vec<PathBuf>> {
        let mut files = self.files()?;
        files.retain(|f| f.extension().is_some_and(|e| e == ext));
        Ok(files)
    }

    /// The files and directories directly in this directory, sorted, the toolchain's [`WORK`]
    /// directory aside.
    ///
    /// # Errors
    /// Returns an error if the directory cannot be read.
    fn entries(&self) -> io::Result<Vec<PathBuf>>;

    /// The bytes of the files at or below this path.
    ///
    /// # Errors
    /// Returns an error if the path does not exist or an entry cannot be read.
    fn size(&self) -> io::Result<u64>;
}

impl Tree for Path {
    fn files(&self) -> io::Result<Vec<PathBuf>> {
        let kind = self.symlink_metadata()?.file_type();
        if kind.is_symlink() {
            return Err(Refusal::Link(self.to_path_buf()).into());
        }
        if !kind.is_dir() {
            return Ok(vec![self.to_path_buf()]);
        }
        let mut found = Vec::new();
        for entry in self.entries()? {
            found.extend(entry.files()?);
        }
        Ok(found)
    }

    fn entries(&self) -> io::Result<Vec<PathBuf>> {
        let mut entries = std::fs::read_dir(self)?
            .filter(|e| !matches!(e, Ok(e) if e.file_name() == WORK))
            .map(|e| e.map(|e| e.path()))
            .collect::<io::Result<Vec<_>>>()?;
        entries.sort();
        Ok(entries)
    }

    fn size(&self) -> io::Result<u64> {
        self.files()?.iter().try_fold(0, |total, f| Ok(total + f.metadata()?.len()))
    }
}
