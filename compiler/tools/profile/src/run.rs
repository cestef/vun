//! Running the compiler for a measurement: timed and reporting its timings, or under callgrind.

use std::path::Path;
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use vun_fs::{Captured, Ended};

use crate::callgrind::Profile;
use crate::error::{MeasureError, Written};
use crate::sample::Sample;
use crate::workload::{Phase, Setup, Verb, Workload};

/// The program that counts instructions.
const VALGRIND: &str = "valgrind";
/// The compiler's options that make it report its timings as data.
const TIMINGS: [&str; 3] = ["--timings", "--format", "json"];

/// Where and how the compiler runs.
pub struct Runner<'a> {
    pub exe: &'a Path,
    /// The repository the workloads' files are relative to.
    pub root: &'a Path,
    /// How many threads the compiler uses, the machine's if none. Instructions are repeatable
    /// with one.
    pub threads: Option<usize>,
    /// The backend generating the code of the programs built, the target's if none.
    pub backend: Option<&'a str>,
    /// How long one run may last.
    pub limit: Duration,
}

impl Runner<'_> {
    /// `samples` runs of the compiler timing itself. A cold run starts from nothing each time; a
    /// warm one from what a first run, which is not measured, saved.
    ///
    /// # Errors
    /// Fails when a run does not end well or reports no timings.
    pub fn sample(
        &self,
        workload: &Workload,
        phase: Phase,
        samples: usize,
    ) -> Result<Vec<Sample>, MeasureError> {
        let mut setup = self.setup(workload, phase)?;
        (0..samples)
            .map(|_| {
                if phase == Phase::Cold {
                    setup = self.setup(workload, phase)?;
                }
                if workload.verb == Verb::Run {
                    let started = Instant::now();
                    self.finish(workload.subject(), &mut setup.measured())?;
                    return Ok(Sample::whole(started.elapsed()));
                }
                let output = self.finish(workload.subject(), setup.command().args(TIMINGS))?;
                Sample::read(&String::from_utf8_lossy(&output.stdout))
            })
            .collect()
    }

    /// The instructions of one run of the compiler under callgrind.
    ///
    /// # Errors
    /// Fails when `valgrind` is missing or the run does not end well.
    pub fn count(&self, workload: &Workload, phase: Phase) -> Result<Profile, MeasureError> {
        let setup = self.setup(workload, phase)?;
        let out = setup.file("callgrind.out");
        let compiler = setup.measured();
        let mut counted = Command::new(VALGRIND);
        counted.args(["--tool=callgrind", "--quiet"]).arg(format!("--callgrind-out-file={}", out.display()));
        counted.arg("--").arg(compiler.get_program()).args(compiler.get_args());
        counted.envs(compiler.get_envs().filter_map(|(k, v)| Some((k, v?))));
        self.finish(workload.subject(), &mut counted)?;
        let text = std::fs::read_to_string(out)?;
        text.parse::<Profile>().map_err(MeasureError::Callgrind)
    }

    /// The setup for `phase`: a warm one has had the command run once, and a built program has been built.
    fn setup(&self, workload: &Workload, phase: Phase) -> Result<Setup, MeasureError> {
        let setup = Setup::new(self.exe, self.root, workload, self.threads, self.backend)?;
        if phase != Phase::Cold {
            self.finish(Workload::COMPILER, &mut setup.command())?;
        }
        Ok(setup)
    }

    /// Runs `command` to its end within the limit.
    fn finish(&self, what: &'static str, command: &mut Command) -> Result<Output, MeasureError> {
        let program = if command.get_program() == VALGRIND { VALGRIND } else { what };
        match Captured::within(command, self.limit) {
            Ok(Ended::Exited(output)) if output.status.success() => Ok(output),
            Ok(Ended::Exited(output)) => Err(MeasureError::Failed(what, output.status, Written::of(&output))),
            Ok(Ended::TimedOut(_)) => Err(MeasureError::TimedOut(what, self.limit)),
            Err(error) => Err(MeasureError::Spawn(program, error)),
        }
    }
}
