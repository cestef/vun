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
    Rustc,
    /// LLVM's tool merging what instrumented programs record, from rustup's `llvm-tools`.
    LlvmProfdata,
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
            Self::Rustc => "rustc",
            Self::LlvmProfdata => "llvm-profdata",
        }
    }

    /// Where it is: the cargo running us, or the first on the search path.
    pub(crate) fn path(self) -> Option<PathBuf> {
        match self {
            Self::Cargo => std::env::var_os(CARGO).map(PathBuf::from),
            Self::LlvmProfdata => Self::rustlib(self.name()),
            Self::Jj | Self::Typst | Self::TreeSitter | Self::Rustc => None,
        }
        .or_else(|| Executable::search(self.name()))
    }

    /// The tool `name` of the running Rust toolchain's own LLVM, where rustup's `llvm-tools` puts
    /// it: `lib/rustlib/<host>/bin` of the sysroot.
    fn rustlib(name: &str) -> Option<PathBuf> {
        let print = |what: &str| {
            let out =
                std::process::Command::new(Self::Rustc.path()?).args(["--print", what]).output().ok()?;
            out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        };
        let path =
            PathBuf::from(print("sysroot")?).join("lib/rustlib").join(print("host-tuple")?).join("bin");
        Some(path.join(name).with_extension(std::env::consts::EXE_EXTENSION)).filter(|p| p.exists())
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
