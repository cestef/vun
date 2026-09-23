//! Directory walks in a stable order.

use std::io;
use std::path::{Path, PathBuf};

/// Directory walks on paths.
pub trait Tree {
    /// The files at or below this path, recursively, sorted; a file is its own only entry.
    ///
    /// # Errors
    /// Returns an error if the path does not exist or a directory cannot be read.
    fn files(&self) -> io::Result<Vec<PathBuf>>;

    /// The files and directories directly in this directory, sorted.
    ///
    /// # Errors
    /// Returns an error if the directory cannot be read.
    fn entries(&self) -> io::Result<Vec<PathBuf>>;
}

impl Tree for Path {
    fn files(&self) -> io::Result<Vec<PathBuf>> {
        if self.is_file() {
            return Ok(vec![self.to_path_buf()]);
        }
        let mut found = Vec::new();
        for entry in self.entries()? {
            found.extend(entry.files()?);
        }
        Ok(found)
    }

    fn entries(&self) -> io::Result<Vec<PathBuf>> {
        let mut entries =
            std::fs::read_dir(self)?.map(|e| e.map(|e| e.path())).collect::<io::Result<Vec<_>>>()?;
        entries.sort();
        Ok(entries)
    }
}
