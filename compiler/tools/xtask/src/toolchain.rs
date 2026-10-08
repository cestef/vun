//! The tree of a toolchain (spec 12, section 6): the built `vun` in `bin/`, the checkout's
//! library and licenses beside it. `cargo xtask package` packs one as a release archive,
//! `cargo xtask install` puts one among the user's.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use vun_build::release::{Build, Release};
use vun_build::toolchains::{BIN, COMPONENTS, CONTENTS, MEMOS, Toolchains};
use vun_data::ToData;
use vun_data::json::Json;
use vun_fs::{Edit, Tree};
use vun_hash::Hash;
use vun_span::Version;
use vun_target::Machine;

use crate::release::Crate;

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

    /// Lays the tree at an empty directory: the built `binary` as `bin/<its name>`, what the
    /// checkout ships beside it, `CONTENTS` except the executable and the memos, which priming makes, and the component `libraries`
    /// in `components/`.
    ///
    /// # Errors
    /// Returns an error if the directory cannot be made fresh, the binary copied, or a shipped
    /// file read or written.
    pub(crate) fn lay(&self, binary: &Path, libraries: &[PathBuf]) -> io::Result<()> {
        self.dir.fresh()?;
        let exe = self.dir.join(BIN).join(binary.file_name().unwrap_or_default());
        exe.parent().map(fs::create_dir_all).transpose()?;
        fs::copy(binary, &exe)?;
        for part in CONTENTS.iter().filter(|&&c| c != BIN && c != MEMOS) {
            for file in self.checkout.join(part).files()? {
                let relative = file.strip_prefix(&self.checkout).unwrap_or(&file);
                self.dir.join(relative).put(fs::read(&file)?)?;
            }
        }
        for library in libraries {
            let name = library.file_name().unwrap_or_default();
            self.dir.join(COMPONENTS).join(name).put(fs::read(library)?)?;
        }
        Ok(())
    }

    /// Primes the toolchain laid here: its own executable, `binary` copied into the tree, is run
    /// over the templates of the library beside it, which keeps the libraries' memos in the
    /// tree. Memos name the executable by its contents, so they hold for every copy of it.
    ///
    /// # Errors
    /// Returns an error if the executable cannot be run, or priming ends without success.
    pub(crate) fn prime(&self, binary: &Path) -> io::Result<()> {
        let exe = self.dir.join(BIN).join(binary.file_name().unwrap_or_default());
        let status = Command::new(exe).args(["toolchain", "prime"]).status()?;
        status.success().then_some(()).ok_or_else(|| io::Error::other(Unprimed(status)))
    }

    /// Keeps beside the tree laid here the manifest of the release it stands for: a release of
    /// one machine, whose components are those `crates` built beside `binary`. A toolchain
    /// installed from a checkout then has what one from a release has, and needs no signature,
    /// being as trusted as the executable itself.
    ///
    /// # Errors
    /// Returns an error if a component cannot be packed or the manifest written.
    pub(crate) fn keep_release(&self, version: Version, binary: &Path, crates: &[Crate]) -> io::Result<()> {
        let machine = Machine::RUNNING;
        let components =
            crates.iter().map(|c| Ok(c.staged(binary, machine)?.0)).collect::<io::Result<_>>()?;
        let build = Build { machine, archive: Build::archive_name(machine), hash: self.hash()?, components };
        let release = Release { version, builds: vec![build] };
        self.dir.join(Release::FILE).put(Json(&release.to_data()).to_string())
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

/// Priming ended without success.
#[derive(Debug)]
struct Unprimed(ExitStatus);

impl std::fmt::Display for Unprimed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "priming the toolchain ended with {}", self.0)
    }
}

impl std::error::Error for Unprimed {}
