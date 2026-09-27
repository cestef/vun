//! Fresh directories for tests that work on files.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::Edit;

/// A directory for one test, emptied when created and removed when dropped.
pub struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    /// The directory for the test `name`; see [`Scratch::create`].
    ///
    /// # Panics
    /// Panics if the directory cannot be created.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self::create(name).expect("scratch directory")
    }

    /// A fresh directory named for `name`, this process and this call, so that no other scratch
    /// directory is it.
    ///
    /// # Errors
    /// Returns an error if the directory cannot be created.
    pub fn create(name: &str) -> io::Result<Self> {
        static CREATED: AtomicU64 = AtomicU64::new(0);
        let call = CREATED.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("vun-{}-{call}-{name}", std::process::id()));
        dir.fresh()?;
        Ok(Self { dir })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.dir
    }

    #[must_use]
    pub fn join(&self, path: impl AsRef<Path>) -> PathBuf {
        self.dir.join(path)
    }

    /// Writes `contents` at `path` below the directory and returns where.
    ///
    /// # Panics
    /// Panics if the file cannot be written.
    pub fn write(&self, path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> PathBuf {
        self.put(path, contents).expect("scratch file")
    }

    /// Writes `contents` at `path` below the directory and returns where.
    ///
    /// # Errors
    /// Returns an error if the file cannot be written.
    pub fn put(&self, path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> io::Result<PathBuf> {
        let path = self.join(path);
        path.put(contents)?;
        Ok(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = self.dir.remove();
    }
}
