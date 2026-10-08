//! Where the packages are on this machine. A file is shown under the directory of its package,
//! which no query reads and no saved memo holds: memos saved where a package was hold for it
//! wherever it is.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{OnceLock, RwLock};

use crate::{FileRef, PackageId};

/// The directory each loaded package is shown under, by the database that loaded it: sessions
/// of one process may hold the same package in other places.
#[derive(Default)]
pub struct Locations {
    dirs: RwLock<HashMap<(u64, PackageId), String>>,
    /// The latest directory set for each package, for a file read where no database is known.
    latest: RwLock<HashMap<PackageId, String>>,
}

impl Locations {
    /// The directories of every package of the process.
    pub fn global() -> &'static Self {
        static GLOBAL: OnceLock<Locations> = OnceLock::new();
        GLOBAL.get_or_init(Self::default)
    }

    /// Shows the files of `package` of the database `db` under `dir`.
    ///
    /// # Panics
    /// Panics if a thread panicked while setting.
    pub fn set(&self, db: u64, package: PackageId, dir: impl Into<String>) {
        let dir = dir.into();
        self.latest.write().expect("locations").insert(package, dir.clone());
        self.dirs.write().expect("locations").insert((db, package), dir);
    }

    /// The path `file` is shown as in the database `db`: below its package's directory, or as it
    /// is named where it has none (a loose file, or a package not located yet).
    ///
    /// # Panics
    /// Panics if a thread panicked while setting.
    #[must_use]
    pub fn display_in(&self, db: u64, file: &FileRef) -> String {
        let dirs = self.dirs.read().expect("locations");
        Self::shown(dirs.get(&(db, file.package)), file)
    }

    /// The path `file` is shown as where no database is known: under the directory set last for
    /// its package.
    ///
    /// # Panics
    /// Panics if a thread panicked while setting.
    #[must_use]
    pub fn display(&self, file: &FileRef) -> String {
        Self::shown(self.latest.read().expect("locations").get(&file.package), file)
    }

    fn shown(dir: Option<&String>, file: &FileRef) -> String {
        match dir {
            Some(dir) => Path::new(dir).join(file.path.as_str()).display().to_string(),
            None => file.path.as_str().to_owned(),
        }
    }
}
