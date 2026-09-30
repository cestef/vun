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
    fn files(&self) -> io::Result<Vec<PathBuf>> {
        self.files_except(&|_| false)
    }

    /// The files at or below this path, as [`Tree::files`], leaving out every path `skip` names
    /// and never reading below it: what another tool owns there, such as `node_modules`.
    ///
    /// # Errors
    /// As [`Tree::files`], for the paths not skipped.
    fn files_except(&self, skip: &dyn Fn(&Path) -> bool) -> io::Result<Vec<PathBuf>>;

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

    /// Its absolute path, links resolved; the empty path, which the directory of a file named
    /// relatively in the current one is, names the current directory.
    ///
    /// # Errors
    /// Returns an error if the path does not exist.
    fn resolved(&self) -> io::Result<PathBuf>;
}

impl Tree for Path {
    fn files_except(&self, skip: &dyn Fn(&Path) -> bool) -> io::Result<Vec<PathBuf>> {
        if skip(self) {
            return Ok(Vec::new());
        }
        let kind = self.symlink_metadata()?.file_type();
        if kind.is_symlink() {
            return Err(Refusal::Link(self.to_path_buf()).into());
        }
        if !kind.is_dir() {
            return Ok(vec![self.to_path_buf()]);
        }
        let mut found = Vec::new();
        for entry in self.entries()? {
            found.extend(entry.files_except(skip)?);
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

    fn resolved(&self) -> io::Result<PathBuf> {
        if self.as_os_str().is_empty() { Path::new(".") } else { self }.canonicalize()
    }
}
