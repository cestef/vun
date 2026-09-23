//! Paths from one directory to another.

use std::path::{Component, Path, PathBuf};

/// Relative paths between existing paths.
pub trait Relative {
    /// The path that leads from the directory `base` to this path, both resolved first; `None`
    /// when either does not exist.
    fn relative_to(&self, base: &Path) -> Option<PathBuf>;
}

impl Relative for Path {
    fn relative_to(&self, base: &Path) -> Option<PathBuf> {
        let (target, base) = (self.canonicalize().ok()?, base.canonicalize().ok()?);
        let common = target.components().zip(base.components()).take_while(|(a, b)| a == b).count();
        let up = base.components().skip(common).map(|_| Component::ParentDir);
        let path: PathBuf = up.chain(target.components().skip(common)).collect();
        Some(if path.as_os_str().is_empty() { PathBuf::from(Component::CurDir.as_os_str()) } else { path })
    }
}
