//! The tree of a toolchain (spec 12, section 6): the built `vun` in `bin/`, the checkout's
//! library and licenses beside it. `cargo xtask package` packs one as a release archive,
//! `cargo xtask install` puts one among the user's.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use vun_build::toolchains::{BIN, CONTENTS, Toolchains};
use vun_fs::{Edit, Tree};
use vun_hash::Hash;

/// A toolchain's tree being put together from the checkout, in a staging directory.
pub(crate) struct Stage {
    checkout: PathBuf,
    dir: PathBuf,
}

impl Stage {
    /// The tree of the checkout `checkout`, built in `dir`.
    pub(crate) fn at(checkout: impl Into<PathBuf>, dir: impl Into<PathBuf>) -> Self {
        Self { checkout: checkout.into(), dir: dir.into() }
    }

    /// Lays the tree at an empty directory: the built `binary` as `bin/<its name>`, and what the
    /// checkout ships beside it, `CONTENTS` except the executable.
    ///
    /// # Errors
    /// Returns an error if the directory cannot be made fresh, the binary copied, or a shipped
    /// file read or written.
    pub(crate) fn lay(&self, binary: &Path) -> io::Result<()> {
        self.dir.fresh()?;
        let exe = self.dir.join(BIN).join(binary.file_name().unwrap_or_default());
        exe.parent().map(fs::create_dir_all).transpose()?;
        fs::copy(binary, &exe)?;
        for part in CONTENTS.iter().filter(|&&c| c != BIN) {
            for file in self.checkout.join(part).files()? {
                let relative = file.strip_prefix(&self.checkout).unwrap_or(&file);
                self.dir.join(relative).put(fs::read(&file)?)?;
            }
        }
        Ok(())
    }

    /// The hash of the tree laid here, as a release names it.
    ///
    /// # Errors
    /// As [`Toolchains::hash`].
    pub(crate) fn hash(&self) -> io::Result<Hash> {
        Toolchains::hash(&self.dir)
    }

    /// Where it is being laid.
    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }
}
