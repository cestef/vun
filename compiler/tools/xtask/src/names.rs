//! `cargo xtask names`: a source file or directory is named for its concern. Words that name no
//! concern (`utils`, `types`) invite a grab bag, so no module is called one of them.

use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};

use vun_variants::Variants;

use crate::lines::CHECKED;

/// A name no module has.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Variants)]
pub(crate) enum Banned {
    Utils,
    Helpers,
    Common,
    Misc,
    Types,
    Manager,
    Service,
    Handler,
}

impl Banned {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Utils => "utils",
            Self::Helpers => "helpers",
            Self::Common => "common",
            Self::Misc => "misc",
            Self::Types => "types",
            Self::Manager => "manager",
            Self::Service => "service",
            Self::Handler => "handler",
        }
    }

    fn named(word: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|banned| banned.name() == word)
    }
}

/// Paths that keep a banned name because it is a public diagnostic group: its ids (`types.arity`) are
/// the file stem, so renaming it renames them.
const KEPT: &[&str] = &["lib/lang/lang/errors/types.vun", "compiler/base/diag/src/errors/types.rs"];

/// A file or directory, relative to the repository root, named by a [`Banned`] word.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Unnamed {
    pub(crate) path: PathBuf,
    pub(crate) banned: Banned,
}

impl Unnamed {
    /// Every module file and every directory holding one under `root` with a banned name, in path order.
    ///
    /// # Errors
    /// Returns an error if a checked directory cannot be read.
    pub(crate) fn all(root: &Path) -> io::Result<Vec<Self>> {
        let mut found = BTreeSet::new();
        for checked in CHECKED.iter().filter(|c| c.modules) {
            for file in checked.files(root)? {
                let relative = file.strip_prefix(root).unwrap_or(&file);
                for (depth, ancestor) in relative.ancestors().enumerate() {
                    let word = if depth == 0 { ancestor.file_stem() } else { ancestor.file_name() };
                    let banned = word.and_then(|w| w.to_str()).and_then(Banned::named);
                    let kept = KEPT.iter().any(|path| ancestor == Path::new(path));
                    found.extend(
                        banned.filter(|_| !kept).map(|banned| Self { path: ancestor.to_path_buf(), banned }),
                    );
                }
            }
        }
        Ok(found.into_iter().collect())
    }
}
