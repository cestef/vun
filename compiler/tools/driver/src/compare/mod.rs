//! Running a program through every backend (spec 13, section 8): each that generates code for the
//! target builds and runs it, and they must agree on what it does. The backend tests check it,
//! and `vun debug compare` shows it: what each backend cost, and where they part.

mod diff;
#[cfg(test)]
mod tests;

use std::process::Output;
use std::time::{Duration, Instant};

pub use diff::{Difference, Stream};
use vun_backend::Backend;
use vun_diag::{Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_fs::Captured;
use vun_intern::Symbol;
use vun_span::FileId;

use crate::Session;

/// What a program did under one backend.
pub struct Ran {
    pub backend: &'static str,
    pub output: Output,
    /// Size of the program built, in bytes.
    pub size: u64,
    /// Time to generate code and link.
    pub built: Duration,
    /// Time of each run, in the order they ran; the first run's output is `output`.
    pub runs: Vec<Duration>,
}

impl Ran {
    /// Whether `runs` all ended alike and printed the same.
    #[must_use]
    pub fn agree(runs: &[Self]) -> bool {
        runs.split_first().is_none_or(|(reference, rest)| rest.iter().all(|r| r.differs(reference).is_none()))
    }

    /// The middle one of the run times.
    #[must_use]
    pub fn median(&self) -> Duration {
        let mut sorted = self.runs.clone();
        sorted.sort_unstable();
        sorted.get(sorted.len() / 2).copied().unwrap_or_default()
    }
}

impl Session {
    /// Builds the program rooted at `file` through each backend that generates code for its
    /// target, in the registry's order, and runs each `times` times.
    ///
    /// # Errors
    /// Returns once what stopped a build or a run is reported.
    ///
    /// # Panics
    /// Never: naming a file chooses the target.
    pub fn run_each(&mut self, file: FileId, times: usize) -> Result<Vec<Ran>, ErrorGuaranteed> {
        self.run_where(file, times, |_| true)
    }

    /// [`Session::run_each`] through the backends `keep` holds, for a program some backend
    /// cannot express and no other assists.
    ///
    /// # Errors
    /// Returns the first backend that fails to build it, or a program that cannot run.
    ///
    /// # Panics
    /// Never: naming a file chooses the target.
    pub fn run_where(
        &mut self,
        file: FileId,
        times: usize,
        keep: impl Fn(&dyn Backend) -> bool,
    ) -> Result<Vec<Ran>, ErrorGuaranteed> {
        let _ = self.lir(file);
        let target = self.target.clone().expect("naming a file chooses the target")?;
        let backends: Vec<&'static str> =
            self.registry.fitting(&target).filter(|&b| keep(b)).map(|b| b.info().id.0).collect();
        let asked = self.codegen;
        let mut ran = Vec::new();
        for backend in backends {
            self.codegen.backend = Some(Symbol::intern(backend));
            let started = Instant::now();
            let program = self.build(file, None);
            let built = started.elapsed();
            self.codegen = asked;
            let program = program?;
            let size = std::fs::metadata(&program).map_or(0, |m| m.len());
            let mut outputs = Vec::new();
            let mut runs = Vec::new();
            for _ in 0..times.max(1) {
                let started = Instant::now();
                let output = Captured::run(&mut self.command(&program)).map_err(|e| {
                    let msg = Msg::new().text("cannot run ").code(program.display().to_string());
                    self.diag.error(Diagnostic::error(errors::BACKEND_TOOL, msg).caused_by(&e))
                })?;
                runs.push(started.elapsed());
                outputs.push(output);
            }
            let output = outputs.swap_remove(0);
            ran.push(Ran { backend, output, size, built, runs });
        }
        Ok(ran)
    }
}
