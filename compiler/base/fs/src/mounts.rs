//! The file systems mounted on this machine.

use std::path::PathBuf;

/// Where file systems are mounted.
pub struct Mounts;

impl Mounts {
    /// The mount table of the system, a file whose lines name the mount point second.
    #[cfg(target_os = "linux")]
    const TABLE: &'static str = "/proc/self/mounts";
    /// A byte of a mount point is written as a backslash and its value in octal where it is a
    /// space, a tab, a newline or a backslash.
    #[cfg(target_os = "linux")]
    const ESCAPE: u8 = b'\\';

    /// The mount points, in the order the system lists them; none where this system's way of
    /// listing them is not known.
    #[must_use]
    #[cfg(target_os = "linux")]
    pub fn all() -> Vec<PathBuf> {
        let table = std::fs::read(Self::TABLE).unwrap_or_default();
        let lines = table.split(|&b| b == b'\n');
        lines.filter_map(|line| line.split(|&b| b == b' ').nth(1)).map(Self::unescaped).collect()
    }

    /// Volumes are the entries of one directory.
    #[must_use]
    #[cfg(target_os = "macos")]
    pub fn all() -> Vec<PathBuf> {
        crate::Tree::entries(std::path::Path::new("/Volumes")).unwrap_or_default()
    }

    /// Volumes are the drive letters that exist.
    #[must_use]
    #[cfg(target_os = "windows")]
    pub fn all() -> Vec<PathBuf> {
        ('A'..='Z').map(|letter| PathBuf::from(format!("{letter}:\\"))).filter(|root| root.exists()).collect()
    }

    #[must_use]
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    pub fn all() -> Vec<PathBuf> {
        Vec::new()
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn unescaped(field: &[u8]) -> PathBuf {
        use std::os::unix::ffi::OsStringExt;
        let mut bytes = Vec::with_capacity(field.len());
        let mut rest = field;
        while let Some((&first, tail)) = rest.split_first() {
            let value = (first == Self::ESCAPE)
                .then(|| tail.get(..3))
                .flatten()
                .and_then(|digits| std::str::from_utf8(digits).ok())
                .and_then(|digits| u8::from_str_radix(digits, 8).ok());
            if let Some(byte) = value {
                bytes.push(byte);
                rest = &tail[3..];
            } else {
                bytes.push(first);
                rest = tail;
            }
        }
        PathBuf::from(std::ffi::OsString::from_vec(bytes))
    }
}
