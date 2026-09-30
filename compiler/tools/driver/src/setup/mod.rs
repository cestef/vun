//! What a build for a target is made with, and where each part is declared (spec 12, section
//! 6): the backend and the linker and why they are the ones, and the programs they run, each
//! found on the machine or missing, the machine's tools file replacing the target's where it
//! names them. `vun target show` and `vun target list` show it, resolved as a build resolves it.

use std::path::{Path, PathBuf};

use vun_build::tools::TargetTools;
use vun_diag::ErrorGuaranteed;
use vun_intern::Symbol;
use vun_sema::target::{Choice, Chosen, Listed, TargetPath, Targets};
use vun_span::{FileId, Span};
use vun_target::{BuildsFor, Machine, Run, TargetSpec};
use vun_tool::Executable;

use crate::Session;
use crate::registry::{Codegen, Linkage, Pick, Registry, Unchosen};

/// A build's parts for one target.
pub struct Setup {
    /// The target, with the machine's tools.
    pub spec: TargetSpec,
    pub backend: Result<(Symbol, Pick), Unchosen>,
    pub linkage: Linkage,
    /// What the backend and the linker run, each once.
    pub programs: Vec<Program>,
    pub sysroot: Option<(Symbol, From)>,
    /// Where programs built for the target run.
    pub runs: Runs,
}

/// A program a build runs.
pub struct Program {
    pub run: Run,
    pub command: Symbol,
    pub from: From,
    pub found: Found,
}

/// Whether the machine has a program a build runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Found {
    /// On the search path, here.
    At(PathBuf),
    /// On the search path, but it builds for the machine running it, which the target is not
    /// for: the machine's tools file names none in its place.
    Host(PathBuf),
    Missing,
}

/// Where programs built for a target run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Runs {
    /// On this machine, as they are.
    Here,
    /// Through the runner the machine's tools file names, an emulator: its command, whether the
    /// machine has its program, and the entry naming it.
    Through { command: Vec<Symbol>, found: Found, at: Span },
    /// Not on this machine: they are built only.
    Elsewhere,
}

/// Where a part of a build is declared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum From {
    /// The target's declaration.
    Target,
    /// The entry of the machine's tools file at this span.
    Tools(Span),
    /// The backend or linker of this name, which runs it.
    Component(Symbol),
}

/// How the target of a session is chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chose {
    /// Named by `--target`.
    Named,
    /// The first of `targets.hosts` for the running machine.
    Host,
    /// A program of a package's plan.
    Plan,
}

/// A target a program may be built for.
pub struct Offered {
    pub listed: Listed,
    pub setup: Setup,
    /// Whether it is the target builds are for unless told otherwise.
    pub host: bool,
}

/// The target of a session: its path, when it has one, how it is chosen, and its build's parts.
pub struct Shown {
    pub path: Option<TargetPath>,
    pub chose: Chose,
    pub setup: Setup,
}

impl Setup {
    /// The parts of a build of `spec`, as declared, equipped with `tools`, as `codegen` asks.
    #[must_use]
    pub fn resolve(
        registry: &Registry,
        codegen: Codegen,
        mut spec: TargetSpec,
        tools: Option<&TargetTools>,
    ) -> Self {
        if let Some(tools) = tools {
            tools.apply(&mut spec);
        }
        let backend = registry.choose(codegen, &spec).map(|p| (p.backend.info().id.symbol(), p.by));
        let linkage = registry.linkage(codegen, &spec);
        let by_backend = backend.as_ref().ok().and_then(|&(name, _)| registry.backend(name)).map(|b| {
            let name = b.info().id.symbol();
            b.info().runs.iter().map(move |&run| (run, name)).collect::<Vec<_>>()
        });
        let by_linker = registry.linker(linkage.linker).map(|l| {
            let name = l.info().id.symbol();
            l.runs(&spec).into_iter().map(move |run| (run, name)).collect::<Vec<_>>()
        });
        let mut programs: Vec<Program> = Vec::new();
        for (run, component) in by_backend.into_iter().chain(by_linker).flatten() {
            if programs.iter().any(|p| p.run == run) {
                continue;
            }
            let from = Self::from(run, component, tools);
            let command = run.command(&spec);
            let found = match Executable::search(command.as_str()) {
                None => Found::Missing,
                Some(path) if Self::builds_for_host(run, &spec) => Found::Host(path),
                Some(path) => Found::At(path),
            };
            programs.push(Program { run, command, from, found });
        }
        let sysroot = spec.c.compiler.sysroot.map(|root| {
            let named = tools.filter(|t| t.sysroot.is_some());
            (root, named.map_or(From::Target, |t| From::Tools(t.span)))
        });
        let runs = match tools.filter(|t| !t.runner.is_empty()) {
            Some(tools) => {
                let found = Executable::search(tools.runner[0].as_str()).map_or(Found::Missing, Found::At);
                Runs::Through { command: tools.runner.clone(), found, at: tools.span }
            }
            None if Machine::RUNNING.is(&spec) => Runs::Here,
            None => Runs::Elsewhere,
        };
        Self { spec, backend, linkage, programs, sysroot, runs }
    }

    /// Whether `run` is a C compiler building for the machine running it, which `spec` is not
    /// for.
    fn builds_for_host(run: Run, spec: &TargetSpec) -> bool {
        run == Run::Compiler && spec.c.compiler.builds_for == BuildsFor::Host && !Machine::RUNNING.is(spec)
    }

    /// Where the command of `run`, which `component` runs, is declared.
    fn from(run: Run, component: Symbol, tools: Option<&TargetTools>) -> From {
        let named = |tools: &TargetTools| match run {
            Run::Compiler => tools.compiler.is_some(),
            Run::Archiver => tools.archiver.is_some(),
            Run::Program(_) => false,
        };
        match (run, tools.filter(|t| named(t))) {
            (_, Some(tools)) => From::Tools(tools.span),
            (Run::Program(_), None) => From::Component(component),
            (Run::Compiler | Run::Archiver, None) => From::Target,
        }
    }

    /// The programs it runs that the machine lacks, or has only for itself.
    pub fn lacking(&self) -> impl Iterator<Item = &Program> {
        self.programs.iter().filter(|p| !matches!(p.found, Found::At(_)))
    }

    /// Whether a build can run: a backend is chosen and every program it runs is here.
    #[must_use]
    pub fn ready(&self) -> bool {
        self.backend.is_ok() && self.lacking().next().is_none()
    }
}

/// The path of a file holding nothing, outside any package: what a program with no root file
/// sees of the targets.
const EMPTY: &str = "<empty>";

impl Session {
    /// A file holding nothing, outside any package, for commands that need no root file.
    pub fn empty_file(&mut self) -> FileId {
        self.open(Path::new(EMPTY), String::new())
    }

    /// The target of a program rooted at `file`, as the session chooses it, and its build's parts.
    ///
    /// # Errors
    /// Returns once no target being chosen is reported.
    ///
    /// # Panics
    /// Never: naming the file chooses the target, whose choice is then memoized.
    pub fn shown(&mut self, file: FileId) -> Result<Shown, ErrorGuaranteed> {
        let key = self.file_ref(file);
        self.target.clone().expect("naming a file chooses the target")?;
        let declared =
            self.db.get::<Chosen>(&(key.clone(), self.choice.clone())).expect("the target is chosen");
        let (path, chose) = match &self.choice {
            Choice::Path(path) => (Some(path.clone()), Chose::Named),
            Choice::Host => (self.target_path(&key, &declared.spec), Chose::Host),
            Choice::Given(_) => (self.target_path(&key, &declared.spec), Chose::Plan),
        };
        let setup = Setup::resolve(&self.registry, self.codegen, declared.spec, self.equipped.as_ref());
        Ok(Shown { path, chose, setup })
    }

    /// Every target a program rooted at `file` may be built for, with its build's parts.
    pub fn setups(&mut self, file: FileId) -> Vec<Offered> {
        let key = self.file_ref(file);
        let host = self.db.get::<Chosen>(&(key.clone(), Choice::Host)).ok();
        let listed = self.db.get::<Targets>(&key);
        listed
            .into_iter()
            .map(|listed| {
                let spec = &listed.target.spec;
                let tools = self.tools_for(&key, spec);
                let setup = Setup::resolve(&self.registry, self.codegen, spec.clone(), tools.as_ref());
                let host = host.as_ref().is_some_and(|h| h.spec == *spec);
                Offered { listed, setup, host }
            })
            .collect()
    }

    /// The path of the target `spec` is, as declared.
    fn target_path(&self, key: &vun_hir::db::FileRef, spec: &TargetSpec) -> Option<TargetPath> {
        let listed = self.db.get::<Targets>(key);
        listed.into_iter().find(|l| l.target.spec == *spec).map(|l| l.path)
    }
}
