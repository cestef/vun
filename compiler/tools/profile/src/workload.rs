//! What is measured: a program of the corpus and the command that takes it through the compiler,
//! and the state of the compiler's saved work when it starts.

use std::path::{Path, PathBuf};
use std::process::Command;

use vun_build::store::Store;
use vun_data::{FromData, ToData};
use vun_fs::{Scratch, Tree};
use vun_par::Usage;
use vun_variants::Variants;

/// What the compiler has saved when a run starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Variants, ToData, FromData)]
pub enum Phase {
    /// Nothing: no memos of the libraries and none of the program.
    Cold,
    /// What an earlier run of the same command saved.
    Warm,
    /// A program built beforehand: the compiler's saved work is not what runs.
    Run,
}

impl Phase {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Cold => "cold",
            Self::Warm => "warm",
            Self::Run => "run",
        }
    }

    #[must_use]
    pub fn names() -> Vec<&'static str> {
        Self::ALL.iter().map(|p| p.name()).collect()
    }

    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|p| p.name() == name)
    }
}

/// The command a workload runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verb {
    /// Analyze without generating code.
    Check,
    /// Generate code and link.
    Build,
    /// Build the program, then measure the program rather than the compiler.
    Run,
}

impl Verb {
    const fn word(self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::Build | Self::Run => "build",
        }
    }
}

/// How long a workload takes, which decides whether a run measures it unless it is named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weight {
    Light,
    /// Seconds cold, which callgrind stretches to minutes.
    Heavy,
}

/// A program and the command run over it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Workload {
    pub name: &'static str,
    /// A source file or a package's directory, relative to the root of the repository.
    pub path: &'static str,
    pub verb: Verb,
    pub weight: Weight,
}

impl Workload {
    pub const ALL: &'static [Self] = &[
        Self::of("check-hello", "corpus/accept/programs/hello.vun", Verb::Check, Weight::Light),
        Self::of("check-tour", "corpus/accept/programs/tour.vun", Verb::Check, Weight::Light),
        Self::of("check-threads", "corpus/accept/programs/threads.vun", Verb::Check, Weight::Light),
        Self::of("check-comptime", "corpus/bench/comptime.vun", Verb::Check, Weight::Light),
        Self::of("build-hello", "corpus/accept/programs/hello.vun", Verb::Build, Weight::Light),
        Self::of("build-tour", "corpus/accept/programs/tour.vun", Verb::Build, Weight::Light),
        Self::of("run-fib", "corpus/bench/fib.vun", Verb::Run, Weight::Light),
        Self::of("run-sieve", "corpus/bench/sieve.vun", Verb::Run, Weight::Light),
        Self::of("run-mix", "corpus/bench/mix.vun", Verb::Run, Weight::Light),
        Self::of("check-std", "lib/std", Verb::Check, Weight::Heavy),
    ];

    /// What a run of the compiler is called in a failure.
    pub const COMPILER: &'static str = "the compiler";

    /// What is measured: the compiler, or the program it built.
    #[must_use]
    pub fn subject(&self) -> &'static str {
        if self.verb == Verb::Run { "the program" } else { Self::COMPILER }
    }

    const fn of(name: &'static str, path: &'static str, verb: Verb, weight: Weight) -> Self {
        Self { name, path, verb, weight }
    }

    /// The workloads a run measures when none is named.
    pub fn defaults() -> impl Iterator<Item = &'static Self> {
        Self::ALL.iter().filter(|w| w.weight == Weight::Light)
    }

    /// The phases to measure: the one asked for or both of the compiler's; a built program has
    /// just its own.
    #[must_use]
    pub fn phases(&self, asked: Option<Phase>) -> Vec<Phase> {
        if self.verb == Verb::Run {
            return vec![Phase::Run];
        }
        asked.map_or_else(|| vec![Phase::Cold, Phase::Warm], |phase| vec![phase])
    }

    #[must_use]
    pub fn names() -> Vec<&'static str> {
        Self::ALL.iter().map(|w| w.name).collect()
    }

    #[must_use]
    pub fn named(name: &str) -> Option<&'static Self> {
        Self::ALL.iter().find(|w| w.name == name)
    }
}

/// How the compiler is started for a workload: its program copied into a directory of its own,
/// where its saved work goes, and the store beside it, so that no run reads what another saved.
pub struct Setup {
    scratch: Scratch,
    program: PathBuf,
    verb: Verb,
    exe: PathBuf,
    threads: Option<usize>,
    backend: Option<String>,
}

impl Setup {
    /// The directory in the store's place, in the scratch directory.
    const STORE: &'static str = "store";
    /// The program a `Verb::Run` workload builds.
    const ARTIFACT: &'static str = "program";

    /// # Errors
    /// Fails when the program cannot be read or copied.
    pub fn new(
        exe: &Path,
        root: &Path,
        workload: &Workload,
        threads: Option<usize>,
        backend: Option<&str>,
    ) -> std::io::Result<Self> {
        let scratch = Scratch::create(workload.name)?;
        let source = root.join(workload.path);
        let name = source.file_name().unwrap_or(source.as_os_str());
        let program = scratch.join(name);
        for file in source.files()? {
            let destination = match file.strip_prefix(&source) {
                Ok(within) if !within.as_os_str().is_empty() => Path::new(name).join(within),
                _ => Path::new(name).to_path_buf(),
            };
            scratch.put(destination, std::fs::read(&file)?)?;
        }
        Ok(Self {
            scratch,
            program,
            verb: workload.verb,
            exe: exe.to_path_buf(),
            threads,
            backend: backend.map(str::to_owned),
        })
    }

    /// The compiler's command line, to which options are added.
    #[must_use]
    pub fn command(&self) -> Command {
        let mut command = Command::new(&self.exe);
        command.arg(self.verb.word()).arg(&self.program).env(Store::ENV, self.scratch.join(Self::STORE));
        if let Some(threads) = self.threads {
            command.env(Usage::VAR, threads.to_string());
        }
        if self.verb == Verb::Run {
            command.arg("--release").arg("--output").arg(self.scratch.join(Self::ARTIFACT));
            if let Some(backend) = &self.backend {
                command.arg("--backend").arg(backend);
            }
        }
        command
    }

    /// What is measured: the compiler's command, or the program it built.
    #[must_use]
    pub fn measured(&self) -> Command {
        match self.verb {
            Verb::Run => Command::new(self.scratch.join(Self::ARTIFACT)),
            Verb::Check | Verb::Build => self.command(),
        }
    }

    /// A file in the setup's directory.
    #[must_use]
    pub fn file(&self, name: &str) -> PathBuf {
        self.scratch.join(name)
    }

    /// The program the compiler is given: the copied file, or the copied package's directory.
    #[must_use]
    pub fn program(&self) -> &Path {
        &self.program
    }
}
