//! Running a program through every backend (spec 13, section 8): each that generates code for the
//! target builds and runs it, and they must agree on what it does. The backend tests check it,
//! and `vun toolchain compare` shows it.

use std::process::Output;

use vun_diag::{Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_intern::Symbol;
use vun_span::FileId;

use crate::Session;

/// What a program did under one backend.
pub struct Ran {
    pub backend: &'static str,
    pub output: Output,
}

impl Ran {
    /// Whether `runs` all ended alike and printed the same.
    #[must_use]
    pub fn agree(runs: &[Self]) -> bool {
        runs.windows(2).all(|w| {
            w[0].output.status.code() == w[1].output.status.code() && w[0].output.stdout == w[1].output.stdout
        })
    }
}

impl Session {
    /// Builds and runs the program rooted at `file` through each backend that generates code
    /// for its target, in the registry's order.
    ///
    /// # Errors
    /// Returns once what stopped a build or a run is reported.
    ///
    /// # Panics
    /// Never: naming a file chooses the target.
    pub fn run_each(&mut self, file: FileId) -> Result<Vec<Ran>, ErrorGuaranteed> {
        let _ = self.lir(file);
        let target = self.target.clone().expect("naming a file chooses the target")?;
        let backends: Vec<&'static str> = self.registry.fitting(&target).map(|b| b.info().id.0).collect();
        let asked = self.codegen;
        let mut runs = Vec::new();
        for backend in backends {
            self.codegen.backend = Some(Symbol::intern(backend));
            let program = self.build(file, None);
            self.codegen = asked;
            let program = program?;
            let output = std::process::Command::new(&program).output().map_err(|e| {
                let msg = Msg::new().text("cannot run ").code(program.display().to_string());
                self.diag.error(Diagnostic::error(errors::BACKEND_TOOL, msg).caused_by(&e))
            })?;
            runs.push(Ran { backend, output });
        }
        Ok(runs)
    }
}
