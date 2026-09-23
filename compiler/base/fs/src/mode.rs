//! The one file mode a package keeps: whether a file is executable. Elsewhere than Unix, no file is.

use std::io;
use std::path::Path;

/// Whether a file runs as a program.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Mode {
    #[default]
    Regular,
    Executable,
}

impl Mode {
    /// The permission bits archives record for it: `rw-r--r--`, or `rwxr-xr-x`.
    #[must_use]
    pub const fn bits(self) -> u32 {
        match self {
            Self::Regular => 0o644,
            Self::Executable => 0o755,
        }
    }

    /// The mode of permission bits: executable when anyone may execute.
    #[must_use]
    pub const fn of_bits(bits: u32) -> Self {
        if bits & EXECUTE == 0 { Self::Regular } else { Self::Executable }
    }
}

/// File modes on paths.
pub trait Modes {
    /// The mode of this file; a file that cannot be read is regular.
    fn mode(&self) -> Mode;

    /// Gives this file `mode`: executable for everyone who can read it, or for no one.
    ///
    /// # Errors
    /// Returns an error if the permissions cannot be read or written.
    fn set_mode(&self, mode: Mode) -> io::Result<()>;
}

#[cfg(unix)]
impl Modes for Path {
    fn mode(&self) -> Mode {
        use std::os::unix::fs::PermissionsExt;
        let executable = self.metadata().is_ok_and(|m| m.permissions().mode() & EXECUTE != 0);
        if executable { Mode::Executable } else { Mode::Regular }
    }

    fn set_mode(&self, mode: Mode) -> io::Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = self.metadata()?.permissions();
        let bits = permissions.mode();
        // Execute follows read: `r--` becomes `r-x`.
        let readable = (bits & READ) >> 2;
        permissions.set_mode(match mode {
            Mode::Regular => bits & !EXECUTE,
            Mode::Executable => bits | readable,
        });
        std::fs::set_permissions(self, permissions)
    }
}

/// The execute bits of user, group and others.
const EXECUTE: u32 = 0o111;
/// The read bits of user, group and others.
#[cfg(unix)]
const READ: u32 = 0o444;

#[cfg(not(unix))]
impl Modes for Path {
    fn mode(&self) -> Mode {
        Mode::Regular
    }

    fn set_mode(&self, _: Mode) -> io::Result<()> {
        Ok(())
    }
}
