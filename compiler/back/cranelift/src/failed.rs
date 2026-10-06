//! Why a function has no code.

use std::fmt;

use cranelift_codegen::CodegenError;
use vun_backend::Lack;
use vun_diag::{Diagnostic, Msg, errors};
use vun_origin::OriginId;
use vun_target::Name;

#[derive(Debug)]
pub(crate) enum Failed {
    /// LIR holds what the backend cannot express.
    Lacks(Lack),
    /// Cranelift refused the function, which it finds too big or beyond the processor.
    Refused { at: OriginId, error: CodegenError },
}

impl From<Lack> for Failed {
    fn from(lack: Lack) -> Self {
        Self::Lacks(lack)
    }
}

impl fmt::Display for Failed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lacks(_) => f.write_str("the program holds what the backend cannot express"),
            Self::Refused { .. } => f.write_str("Cranelift refused a function"),
        }
    }
}

impl std::error::Error for Failed {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Lacks(_) => None,
            Self::Refused { error, .. } => Some(error),
        }
    }
}

impl Failed {
    pub(crate) fn diagnostic(&self, backend: Name) -> Diagnostic {
        match self {
            Self::Lacks(lack) => lack.diagnostic(backend),
            Self::Refused { at, error } => {
                let msg = Msg::new().text("backend ").code(backend.0).text(" cannot compile this function");
                Diagnostic::error(errors::BACKEND_LACKS, msg).at(*at, None).caused_by(error)
            }
        }
    }
}
