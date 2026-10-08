//! Priming a toolchain (spec 13, section 2): the libraries' memos it ships, so that the first
//! program a user checks finds them computed. The libraries are checked whole, every file of
//! each, as the dependencies of a program with nothing in it: what they compute is what every
//! program computes alike, so it is shared, and kept at the toolchain's own `memos`.

use std::fmt;
use std::io;
use std::path::PathBuf;

use vun_diag::DiagCtxt;
use vun_fs::{Edit, Scratch, Tree};
use vun_hir::db::{FileRef, Package, Parsed};
use vun_intern::Symbol;
use vun_span::{EXTENSION, FileId};

use crate::Session;

/// The file standing for a program with nothing in it.
const EMPTY: &str = "prime.vun";

/// What is primed: the libraries at `library`, and where the memos are kept.
pub struct Prime {
    pub library: PathBuf,
    pub memos: PathBuf,
}

/// What priming made.
pub struct Primed {
    /// How many source files of the libraries were checked.
    pub files: usize,
    /// How many bytes of memos were kept.
    pub bytes: u64,
    /// What checking them reported: a library that does not check is not shipped primed.
    pub diag: DiagCtxt,
}

/// Why a toolchain is not primed.
#[derive(Debug)]
pub struct PrimeError(io::Error);

impl fmt::Display for PrimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a file of the priming is not read or written")
    }
}

impl std::error::Error for PrimeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl From<io::Error> for PrimeError {
    fn from(error: io::Error) -> Self {
        Self(error)
    }
}

impl Prime {
    /// Checks every file of the libraries and keeps the memos at `memos`, replacing what was
    /// there.
    ///
    /// # Errors
    /// Returns an error if the memos directory cannot be made fresh or read.
    pub fn run(&self) -> Result<Primed, PrimeError> {
        let scratch = Scratch::create("prime")?;
        self.memos.fresh()?;
        let mut session = Session::new();
        session.library.clone_from(&self.library);
        session.shared = Some(self.memos.clone());
        let root = session.open(&scratch.join(EMPTY), String::new());
        let files = session.check_libraries(root);
        session.share();
        let diag = std::mem::take(&mut session.diag);
        let bytes = self.memos.files()?.iter().map(|file| file.metadata().map_or(0, |m| m.len())).sum();
        Ok(Primed { files, bytes, diag })
    }
}

impl Session {
    /// Checks every source file of the libraries the program of `root` builds on, as `check`
    /// does a file loaded; the number checked.
    pub fn check_libraries(&mut self, root: FileId) -> usize {
        let package = self.package_of(root);
        self.publish(package);
        let libraries: Vec<_> =
            self.packages.published.iter().filter(|&&key| self.packages.library(key)).collect();
        let ids: Vec<_> = libraries.iter().map(|key| self.packages.list[key].id).collect();
        let mut files = Vec::new();
        for package in ids {
            let data = self.db.get::<Package>(&package);
            let sources = data.files.iter().filter(|path| path.ends_with(EXTENSION));
            files.extend(sources.map(|path| FileRef { package, path: Symbol::intern(path) }));
        }
        for key in &files {
            if let Some(parsed) = self.db.get::<Parsed>(key) {
                self.check(parsed.file);
            }
        }
        files.len()
    }
}
