//! One backend's build of a program and its run.

use std::process::Output;
use std::time::Instant;

use vun_fs::{Captured, Ended};
use vun_intern::Symbol;
use vun_span::FileId;

use super::{Death, Finding, Stage, Trying};
use crate::codegen::{Listed, Uncoded};
use crate::compare::{Ending, Ran};

/// A program every backend built and ran alike.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Agreed {
    pub backends: Vec<&'static str>,
    /// The backends that said they cannot express it, which are left out of the agreement.
    pub lacking: Vec<&'static str>,
    /// What it wrote, which the backends agree on.
    pub stdout: Vec<u8>,
}

impl Trying<'_> {
    /// The code of the program rooted at `file` generated through `backend`, and nothing done with it.
    ///
    /// # Errors
    /// Returns why the backend does not generate it.
    pub(super) fn generate(&mut self, file: FileId, backend: &'static str) -> Result<(), Finding> {
        let stage = Stage::Backend(backend);
        let catching = self.catching;
        let session = &mut *self.session;
        let asked = session.codegen;
        session.codegen.backend = Some(Symbol::intern(backend));
        session.diag = vun_diag::DiagCtxt::default();
        let coded = catching.run(|| session.code(file, Listed::No).map(drop));
        session.codegen = asked;
        match coded {
            Err(site) => Err(Finding::Crashed { stage, site }),
            Ok(Ok(())) => Ok(()),
            Ok(Err(Uncoded::Unlowered(unlowered))) => {
                Err(Finding::Unlowered { stage, lacking: super::Lacking::Lir(unlowered.why) })
            }
            Ok(Err(Uncoded::Reported(_))) => Err(Finding::Unbuilt { backend, reports: self.reports() }),
        }
    }

    /// The program rooted at `file` built through `backend` and run for at most the limit.
    ///
    /// # Errors
    /// Returns why it was not built, did not run, or ended other than by success.
    pub(super) fn attempt(&mut self, file: FileId, backend: &'static str) -> Result<Ran, Finding> {
        let stage = Stage::Backend(backend);
        let (catching, limit) = (self.catching, self.trial.limit);
        let session = &mut *self.session;
        let asked = session.codegen;
        session.codegen.backend = Some(Symbol::intern(backend));
        session.diag = vun_diag::DiagCtxt::default();
        let started = Instant::now();
        let built = catching.run(|| session.build(file, None));
        session.codegen = asked;
        let program = match built {
            Err(site) => return Err(Finding::Crashed { stage, site }),
            Ok(Err(_)) => return Err(Finding::Unbuilt { backend, reports: self.reports() }),
            Ok(Ok(program)) => program,
        };
        let built = started.elapsed();
        let size = std::fs::metadata(&program).map_or(0, |m| m.len());
        let started = Instant::now();
        let ended = Captured::within(&mut self.session.command(&program), limit)
            .map_err(|_| Finding::Unbuilt { backend, reports: Vec::new() })?;
        let run = started.elapsed();
        let output = match ended {
            Ended::TimedOut(_) => return Err(Finding::Died { backend, death: Death::Hung }),
            Ended::Exited(output) => output,
        };
        Self::survived(backend, &output)?;
        Ok(Ran { backend, output, size, built, runs: vec![run] })
    }

    /// Whether the run ended by success.
    fn survived(backend: &'static str, output: &Output) -> Result<(), Finding> {
        match Ending::of(&output.status) {
            Ending::Returned(0) => Ok(()),
            Ending::Returned(code) => Err(Finding::Died { backend, death: Death::Exit(code) }),
            Ending::Trapped => Err(Finding::Died { backend, death: Death::Trapped }),
            Ending::Crashed(signal) => Err(Finding::Died { backend, death: Death::Crashed(signal) }),
        }
    }
}
