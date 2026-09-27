//! Carrying out a package's plan (spec 12, section 2): `vun build` on a `build.vun` runs its
//! `build` and builds each program the plan records, in a session of its own, since a session
//! compiles for one target. Steps this toolchain does not carry out yet are warnings.

use std::path::{Path, PathBuf};

use vun_diag::{Diagnostic, ErrorGuaranteed, Msg, Severity, errors};
use vun_link::MemoryRegion;
use vun_origin::OriginId;
use vun_sema::plan::{Action, Options};
use vun_sema::target::Choice;
use vun_span::{FileId, SourceMap};
use vun_trace::tracing;

use crate::{Persistence, Session};

/// A step the toolchain records but does not carry out yet, by what it does.
#[derive(Clone, Copy)]
enum Pending {
    Flash,
    Tests,
    Parameters,
    Lints,
}

impl Pending {
    fn what(self) -> &'static str {
        match self {
            Self::Flash => "writes a program to its device",
            Self::Tests => "runs a program's tests",
            Self::Parameters => "passes parameters to a dependency",
            Self::Lints => "sets lint levels",
        }
    }
}

impl Session {
    /// Carries out the plan of the package whose `build.vun` is `file`, returning the programs
    /// built, in the order the plan records them.
    ///
    /// # Errors
    /// Returns once what stopped it is reported.
    ///
    /// # Panics
    /// Never: a plan that did not run, or a build that stopped, is reported.
    pub fn carry_out(&mut self, file: FileId) -> Result<Vec<PathBuf>, ErrorGuaranteed> {
        let Some(plan) = self.plan(file) else {
            return Err(self.diag.failed().expect("a plan that did not run is reported"));
        };
        let steps = plan.steps().map_err(|(step, why)| {
            let msg = Msg::new()
                .text(format!("step {} of the plan is not a ", step + 1))
                .code(vun_items::std::build::Action.to_string());
            self.diag.error(Diagnostic::error(errors::PLAN_UNREAD, msg).at(plan.at, None).caused_by(&why))
        })?;
        let dir = Path::new(&self.sources.file(file).path).parent().unwrap_or(Path::new("")).to_path_buf();
        let mut built = Vec::new();
        for (step, action) in steps.iter().enumerate() {
            let pending = match action {
                Action::Program(options) => {
                    built.push(self.build_program(&dir, options, plan.at)?);
                    continue;
                }
                Action::Dep { params, .. } if params.is_empty_record() => continue,
                Action::Dep { .. } => Pending::Parameters,
                Action::Flash { .. } => Pending::Flash,
                Action::TestOn { .. } => Pending::Tests,
                Action::Lints(_) => Pending::Lints,
            };
            let msg = Msg::new().text(format!(
                "step {} of the plan {}, which this toolchain does not do yet",
                step + 1,
                pending.what()
            ));
            self.diag
                .report(Diagnostic::new(errors::PLAN_UNSUPPORTED, Severity::Warning, msg).at(plan.at, None));
        }
        Ok(built)
    }

    /// Builds the program `options` describe, its root relative to `dir`, in a session for its
    /// target; the reports of that session become this one's.
    fn build_program(
        &mut self,
        dir: &Path,
        options: &Options,
        at: OriginId,
    ) -> Result<PathBuf, ErrorGuaranteed> {
        let mut session = self.child(Choice::Given(Box::new(options.target.clone())));
        session.memory = options
            .memory
            .iter()
            .map(|r| MemoryRegion { name: r.name, origin: r.origin, len: r.len, declared: at })
            .collect();
        let root = dir.join(options.root.as_str());
        tracing::info!(root = %root.display(), arch = %options.target.spec.arch, "building a program of the plan");
        let result = session.load(&root).ok_or(()).and_then(|file| session.build(file, None).map_err(|_| ()));
        self.diag.absorb(&session.diag);
        result.map_err(|()| self.diag.failed().expect("a build that stopped is reported"))
    }

    /// A session like this one, compiling for `choice`: its settings, none of its state.
    fn child(&self, choice: Choice) -> Session {
        let mut session = Session::new();
        session.format = self.format;
        session.library.clone_from(&self.library);
        session.store = self.store.clone();
        session.schemes = self.schemes.clone();
        session.codegen = self.codegen;
        session.toolchain.clone_from(&self.toolchain);
        session.persistence = match self.persistence {
            Persistence::Off => Persistence::Off,
            Persistence::On | Persistence::At(_) => Persistence::On,
        };
        session.choice = choice;
        session
    }
}
