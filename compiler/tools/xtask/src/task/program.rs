//! The external programs tasks run, each found once, the same way.

use std::path::PathBuf;

use vun_diag::{DiagId, Diagnostic, Msg};
use vun_tool::Executable;

#[derive(Clone, Copy)]
pub(crate) enum Program {
    Cargo,
    Jj,
    Typst,
    TreeSitter,
    Rsync,
    Ssh,
}

/// Where cargo says it is when it runs us.
const CARGO: &str = "CARGO";

const UNAVAILABLE: DiagId = DiagId::new("xtask.program.unavailable");

impl Program {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Cargo => "cargo",
            Self::Jj => "jj",
            Self::Typst => "typst",
            Self::TreeSitter => "tree-sitter",
            Self::Rsync => "rsync",
            Self::Ssh => "ssh",
        }
    }

    /// Where it is: the cargo running us, or the first on the search path.
    pub(crate) fn path(self) -> Option<PathBuf> {
        match self {
            Self::Cargo => std::env::var_os(CARGO).map(PathBuf::from),
            Self::Jj | Self::Typst | Self::TreeSitter | Self::Rsync | Self::Ssh => None,
        }
        .or_else(|| Executable::search(self.name()))
    }

    /// It, ready to run.
    ///
    /// # Errors
    /// Returns a diagnostic when it is not on the search path.
    pub(crate) fn find(self) -> Result<Executable, Box<Diagnostic>> {
        let path = self.path().ok_or_else(|| {
            let msg = Msg::new().code(self.name()).text(" is not on the search path");
            Box::new(Diagnostic::error(UNAVAILABLE, msg))
        })?;
        Ok(Executable::new(self.name(), path))
    }
}
