//! Changes to paths that create what they need and treat an absent path as already removed.

use std::fs;
use std::io;
use std::path::Path;

/// Changes on paths.
pub trait Edit {
    /// Writes `contents` as this file, creating its parent directories.
    ///
    /// # Errors
    /// Returns an error if a directory or the file cannot be written.
    fn put(&self, contents: impl AsRef<[u8]>) -> io::Result<()>;

    /// Removes this directory and everything in it; an absent path is already removed.
    ///
    /// # Errors
    /// Returns an error if something present cannot be removed.
    fn remove(&self) -> io::Result<()>;

    /// Makes this an empty directory, removing what was there.
    ///
    /// # Errors
    /// Returns an error if the old directory cannot be removed or the new one created.
    fn fresh(&self) -> io::Result<()>;

    /// Moves the directory `from` here, replacing what was there and creating the parents.
    ///
    /// # Errors
    /// Returns an error if the old directory cannot be removed or `from` cannot be moved.
    fn replace_with(&self, from: &Path) -> io::Result<()>;
}

impl Edit for Path {
    fn put(&self, contents: impl AsRef<[u8]>) -> io::Result<()> {
        if let Some(parent) = self.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(self, contents)
    }

    fn remove(&self) -> io::Result<()> {
        match fs::remove_dir_all(self) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            done => done,
        }
    }

    fn fresh(&self) -> io::Result<()> {
        self.remove()?;
        fs::create_dir_all(self)
    }

    fn replace_with(&self, from: &Path) -> io::Result<()> {
        if let Some(parent) = self.parent() {
            fs::create_dir_all(parent)?;
        }
        self.remove()?;
        fs::rename(from, self)
    }
}
