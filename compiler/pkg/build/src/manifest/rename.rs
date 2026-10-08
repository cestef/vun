//! Renaming a package: the files of a package are named after it (spec 01, section 8), so a
//! project started from a template renames them with it.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// A package's new name, and the old one its files are named after.
#[derive(Clone, Copy, Debug)]
pub struct Rename<'a> {
    pub from: &'a str,
    pub to: &'a str,
}

impl Rename<'_> {
    /// `path` with each component named after the old name, a directory or a file of any
    /// extension, named after the new one.
    #[must_use]
    pub fn path(&self, path: &Path) -> PathBuf {
        path.components().map(|part| self.component(part.as_os_str())).collect()
    }

    fn component(&self, part: &OsStr) -> OsString {
        let path = Path::new(part);
        if path.file_stem() != Some(OsStr::new(self.from)) {
            return part.to_owned();
        }
        match path.extension() {
            Some(extension) => Path::new(self.to).with_extension(extension).into_os_string(),
            None => OsString::from(self.to),
        }
    }
}
