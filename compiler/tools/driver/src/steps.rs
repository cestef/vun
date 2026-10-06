//! Carrying out a package's plan (spec 12, section 2): `vun build` on a package runs its `build`
//! and builds each program the plan records, in a session of its own, since a session compiles
//! for one target, or only the program rooted at a file named. Steps this toolchain does not
//! carry out yet are warnings.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use vun_diag::{Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_fs::Tree;
use vun_link::{MemoryRegion, SectionPlacement};
use vun_origin::OriginId;
use vun_sema::target::Choice;
use vun_sema_plan::{Action, Delivery, Method, Options, Plan};
use vun_span::{FileId, SourceMap};
use vun_trace::tracing;

use crate::setup::Runs;
use crate::{Persistence, Session};

/// A program built: where it is, where it runs, and the files its plan makes for writing it to
/// a device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Built {
    pub path: PathBuf,
    pub runs: Runs,
    pub flashing: Vec<Flashing>,
}

/// What a plan's flash step made of a program for its device: a file, and the ways to give it to
/// the device, which `Session::deliver` tries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Flashing {
    pub file: PathBuf,
    pub ways: Vec<Delivery>,
}

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
            Self::Flash => "writes a program to its device through a probe or DFU",
            Self::Tests => "runs a program's tests",
            Self::Parameters => "passes parameters to a dependency",
            Self::Lints => "sets lint levels",
        }
    }
}

impl Session {
    /// Carries out the plan of the package whose `build.vun` is `file`, returning the programs
    /// built, in the order the plan records them, with what its flash steps made of each.
    ///
    /// # Errors
    /// Returns once what stopped it is reported.
    ///
    /// # Panics
    /// Never: a plan that did not run, or a build that stopped, is reported.
    pub fn carry_out(&mut self, file: FileId) -> Result<Vec<Built>, ErrorGuaranteed> {
        let (plan, steps, dir) = self.steps(file)?;
        let mut built: Vec<Built> = Vec::new();
        // Where in `built` each program step put what it built, by the step.
        let mut at_step = HashMap::new();
        for (step, action) in steps.iter().enumerate() {
            let pending = match action {
                Action::Program(options) => {
                    let program = self.build_program(&dir, options, plan.at, None)?;
                    at_step.insert(step, built.len());
                    built.push(program);
                    continue;
                }
                Action::Flash { program, method: Method::Uf2 { family, deliver } } => {
                    if let Some(&at) = at_step.get(program) {
                        let file = self.write_uf2(&built[at].path, *family, plan.at)?;
                        built[at].flashing.push(Flashing { file, ways: deliver.clone() });
                    }
                    continue;
                }
                Action::Dep { params, .. } if params.is_empty_record() => continue,
                Action::Dep { .. } => Pending::Parameters,
                Action::Flash { .. } => Pending::Flash,
                Action::TestOn { .. } => Pending::Tests,
                // Read by the commands it sets the options of, not carried out.
                Action::Commands(_) => continue,
                Action::Lints(_) => Pending::Lints,
            };
            let msg = Msg::new()
                .text("step ")
                .num(i128::try_from(step + 1).unwrap_or(i128::MAX))
                .text(" of the plan ")
                .text(pending.what())
                .text(", which this toolchain does not do yet");
            self.diag.report(
                Diagnostic::warning(errors::PLAN_UNSUPPORTED, msg)
                    .at(plan.at, None)
                    .note("the other steps are carried out"),
            );
        }
        Ok(built)
    }

    /// Builds the program the plan of the package whose `build.vun` is `file` records with its
    /// root at `root`, with the options the plan gives it, written to `output` when given;
    /// `None` when the plan records no program rooted there.
    ///
    /// # Errors
    /// Returns once what stopped it is reported.
    pub fn planned(
        &mut self,
        file: FileId,
        root: &Path,
        output: Option<&Path>,
    ) -> Result<Option<Built>, ErrorGuaranteed> {
        let (plan, steps, dir) = self.steps(file)?;
        let wanted = root.resolved().ok();
        let rooted = steps.iter().find_map(|action| match action {
            Action::Program(options) if dir.join(options.root.as_str()).resolved().ok() == wanted => {
                Some(options)
            }
            _ => None,
        });
        rooted.map(|options| self.build_program(&dir, options, plan.at, output)).transpose()
    }

    /// Builds the program rooted at `file` as a program of its own, which no plan describes.
    ///
    /// # Errors
    /// Returns once what stopped it is reported.
    pub fn build_alone(&mut self, file: FileId, output: Option<&Path>) -> Result<Built, ErrorGuaranteed> {
        let path = self.build(file, output)?;
        let runs = self.shown(file)?.setup.runs;
        Ok(Built { path, runs, flashing: Vec::new() })
    }

    /// The plan of the package whose `build.vun` is `file`, its steps, and the directory their
    /// roots are relative to.
    fn steps(&mut self, file: FileId) -> Result<(Plan, Vec<Action>, PathBuf), ErrorGuaranteed> {
        let Some(plan) = self.plan(file) else {
            return Err(self.diag.failed().expect("a plan that did not run is reported"));
        };
        let steps = Action::steps(&plan).map_err(|(step, why)| {
            let msg = Msg::new()
                .text("step ")
                .num(i128::try_from(step + 1).unwrap_or(i128::MAX))
                .text(" of the plan is not a ")
                .code(vun_items::std::build::Action.to_string());
            self.diag.error(Diagnostic::error(errors::PLAN_UNREAD, msg).at(plan.at, None).caused_by(&why))
        })?;
        let dir = Path::new(&self.sources.file(file).path).parent().unwrap_or(Path::new("")).to_path_buf();
        Ok((plan, steps, dir))
    }

    /// The program to run among those `built`: the one there is, else `None` once why none is
    /// is reported.
    pub fn to_run(&mut self, built: &[Built]) -> Option<Built> {
        if let [program] = built {
            return self.runs_here(program).then(|| program.clone());
        }
        let d = if built.is_empty() {
            let msg = Msg::new().text("the plan builds no program to run");
            Diagnostic::error(errors::PLAN_NOTHING_TO_RUN, msg)
                .help(Msg::new().text("name the root file of a program instead"))
        } else {
            let msg = Msg::new()
                .text("the plan builds ")
                .num(i128::try_from(built.len()).unwrap_or(i128::MAX))
                .text(" programs; which to run is not said");
            let d = Diagnostic::error(errors::PLAN_AMBIGUOUS_RUN, msg);
            let d = built.iter().fold(d, |d, program| {
                d.note(Msg::new().text("it builds ").code(crate::subject::Subject::relative(&program.path)))
            });
            d.help(Msg::new().text("name the root file of the program to run"))
        };
        self.diag.error(d);
        None
    }

    /// Whether `program` runs on this machine; if not, why is reported.
    fn runs_here(&mut self, program: &Built) -> bool {
        if program.runs != Runs::Elsewhere {
            return true;
        }
        let msg = Msg::new()
            .code(crate::subject::Subject::relative(&program.path))
            .text(" is built for a device, which this machine does not emulate");
        let help = if program.flashing.is_empty() {
            Msg::new().text("record how it reaches the device with ").code("b.flash")
        } else {
            Msg::new().text("write it to the device with ").code("vun flash")
        };
        self.diag.error(Diagnostic::error(errors::PLAN_RUNS_ELSEWHERE, msg).help(help));
        false
    }

    /// Builds the program `options` describe, its root relative to `dir`, in a session for its
    /// target; the reports of that session become this one's.
    fn build_program(
        &mut self,
        dir: &Path,
        options: &Options,
        at: OriginId,
        output: Option<&Path>,
    ) -> Result<Built, ErrorGuaranteed> {
        let mut session = self.child(Choice::Given(Box::new(options.target.clone())));
        session.codegen.output = options.output;
        session.add_checks(self.checks().iter().chain(options.checks.iter().copied()));
        session.set_trace(options.trace.unwrap_or(self.trace()));
        session.memory = options
            .memory
            .iter()
            .map(|r| MemoryRegion { name: r.name, origin: r.origin, len: r.len, declared: at })
            .collect();
        session.sections = options
            .sections
            .iter()
            .map(|p| SectionPlacement {
                name: SectionPlacement::section(p.name),
                region: p.region,
                load_region: p.load,
                keep: false,
            })
            .collect();
        let root = dir.join(options.root.as_str());
        tracing::info!(root = %root.display(), arch = %options.target.spec.arch, "building a program of the plan");
        let result =
            session.load(&root).ok_or(()).and_then(|file| session.build(file, output).map_err(|_| ()));
        self.diag.absorb(&session.diag);
        let runs = Runs::of(&options.target.spec, session.equipped.as_ref());
        result
            .map(|path| Built { path, runs, flashing: Vec::new() })
            .map_err(|()| self.diag.failed().expect("a build that stopped is reported"))
    }

    /// A session like this one, compiling for `choice`: its settings, none of its state.
    fn child(&self, choice: Choice) -> Session {
        let mut session = Session::new();
        session.format = self.format;
        session.library.clone_from(&self.library);
        session.store = self.store.clone();
        session.codegen = self.codegen;
        session.tools.clone_from(&self.tools);
        session.persistence = match self.persistence {
            Persistence::Off => Persistence::Off,
            Persistence::On | Persistence::At(_) => Persistence::On,
        };
        session.shared.clone_from(&self.shared);
        session.choice = choice;
        session
    }
}
