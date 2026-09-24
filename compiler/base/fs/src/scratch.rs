//! Fresh directories for tests that work on files.

use std::path::{Path, PathBuf};

use super::Edit;

/// A directory for one test, emptied when created and removed when dropped.
pub struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    /// The directory for the test `name`, unique within the running test binary.
    ///
    /// # Panics
    /// Panics if the directory cannot be created.
    #[must_use]
    pub fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("vun-{}-{name}", std::process::id()));
        dir.fresh().expect("scratch directory");
        Self { dir }
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
        let path = self.join(path);
        path.put(contents).expect("scratch file");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = self.dir.remove();
    }
}
