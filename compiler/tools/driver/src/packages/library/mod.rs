//! The library shipped with the toolchain (spec 12, section 6): the default dependencies and the
//! pages explaining diagnostics. An installed toolchain is a directory holding `bin/vun` and the
//! library in `lib/`; a build run from a checkout reads the checkout's.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use vun_build::layout::MANIFEST;
use vun_build::manifest::DEFAULT_DEPS;
use vun_fs::Tree;
use vun_trace::tracing;

#[cfg(test)]
mod tests;

/// The library and where it was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Library {
    pub dir: PathBuf,
    pub found: Found,
}

/// Why the library is where it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Found {
    /// The directory [`Library::ENV`] names.
    Named,
    /// `lib/` of the toolchain holding the running executable.
    Installed,
    /// The checkout the executable was built from: development builds.
    Checkout,
}

impl Library {
    /// The environment variable naming the library, instead of the toolchain's.
    pub const ENV: &str = "VUN_LIBRARY";
    /// The library's directory in a toolchain, beside `bin/`.
    pub const DIR: &str = "lib";
    /// The library of the checkout this crate was built in.
    const CHECKOUT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../lib");

    /// The library of the running toolchain, by its real path, so that its files are named as an
    /// editor names them.
    #[must_use]
    pub fn find() -> Self {
        let exe = vun_fs::EXECUTABLE
            .as_deref()
            .map(|exe| std::fs::canonicalize(exe).unwrap_or_else(|_| exe.to_path_buf()));
        let library = Self::locate(std::env::var_os(Self::ENV), exe.as_deref());
        let dir = std::fs::canonicalize(&library.dir).unwrap_or(library.dir);
        tracing::debug!(dir = %dir.display(), found = ?library.found, "library");
        Self { dir, ..library }
    }

    /// The library for the variable `named` and the executable `exe`: the directory named, else
    /// the toolchain's `lib/` when it holds every default dependency, else the checkout.
    fn locate(named: Option<OsString>, exe: Option<&Path>) -> Self {
        if let Some(dir) = named {
            return Self { dir: dir.into(), found: Found::Named };
        }
        let installed = exe.and_then(|exe| Some(exe.parent()?.parent()?.join(Self::DIR)));
        match installed.filter(|dir| Self::holds_defaults(dir)) {
            Some(dir) => Self { dir, found: Found::Installed },
            None => Self { dir: Self::CHECKOUT.into(), found: Found::Checkout },
        }
    }

    /// The directory of a library holding the templates, each a project `vun new` copies.
    pub const TEMPLATES: &str = "templates";

    /// The template `vun new` copies when none is named.
    pub const DEFAULT_TEMPLATE: &str = "hello";

    /// The names of the templates of the library at `dir`.
    #[must_use]
    pub fn templates(dir: &Path) -> Vec<String> {
        let dirs = dir.join(Self::TEMPLATES).entries().unwrap_or_default();
        dirs.iter().filter(|d| d.is_dir()).filter_map(|d| Some(d.file_name()?.to_str()?.to_owned())).collect()
    }

    fn holds_defaults(dir: &Path) -> bool {
        DEFAULT_DEPS.iter().all(|dep| dir.join(dep).join(MANIFEST).is_file())
    }
}
