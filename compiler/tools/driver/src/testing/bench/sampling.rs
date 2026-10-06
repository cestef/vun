//! Taking the samples of the benchmarks (decision 600): each sample is a run of the tests'
//! program measuring them, so a sample holds what a process does as well as what its loop does.

use std::ffi::OsString;
use std::io;
use std::path::Path;

use vun_fs::Scratch;
use vun_term::{Document, Shell};

use super::counter::{Counted, Counter, Unavailable};
use crate::Session;
use crate::testing::{Collected, Measured, Options, Outcome, Tested};

/// A benchmark's samples, each a measure per iteration.
pub(super) struct Sampled {
    pub name: String,
    pub values: Vec<f64>,
}

/// The samples of the benchmarks of a run, and whether every process ran to its end.
#[derive(Default)]
pub(super) struct Sampling {
    pub benchmarks: Vec<Sampled>,
    pub passed: bool,
}

impl Sampling {
    fn new() -> Self {
        Self { benchmarks: Vec::new(), passed: true }
    }

    fn push(&mut self, name: &str, value: f64) {
        match self.benchmarks.iter_mut().find(|b| b.name == name) {
            Some(sampled) => sampled.values.push(value),
            None => self.benchmarks.push(Sampled { name: name.to_owned(), values: vec![value] }),
        }
    }

    /// Takes the sample each benchmark of `collected` measured, as `value` gives it; reports what
    /// else it did, and whether the program ran to its end and measured.
    fn absorb(
        &mut self,
        collected: &Collected,
        value: impl Fn(Measured) -> f64,
        shell: &mut Shell<'_>,
    ) -> bool {
        for tested in &collected.tests {
            match tested.ran.outcome {
                Outcome::Measured(measured) => self.push(&tested.ran.name, value(measured)),
                _ => shell.status(tested),
            }
        }
        let measured = collected.tests.iter().all(Tested::measured);
        if !collected.output.is_empty() {
            let name = collected.program.display().to_string();
            // Reporting is best effort: the run's failure is what the command ends with.
            let _ = shell.result(&Document { kind: "output", name, text: collected.output.join("\n") });
        }
        if collected.tests.is_empty() {
            let name = collected.program.display().to_string();
            let text = format!("ended its process with {} before it reported", collected.status);
            let _ = shell.result(&Document { kind: "output", name, text });
        }
        let clean = collected.status.success() && measured && !collected.tests.is_empty();
        self.passed &= clean;
        clean
    }
}

impl Tested {
    /// Whether it is a benchmark that was measured.
    fn measured(&self) -> bool {
        matches!(self.ran.outcome, Outcome::Measured(_))
    }
}

impl Session {
    /// `samples` runs of the program measuring the benchmarks `names`, every one if none; each
    /// benchmark's samples are its wall-clock nanoseconds per iteration.
    ///
    /// # Errors
    /// Fails when the program cannot be started or its output read.
    pub(super) fn sample_time(
        &self,
        program: &Path,
        names: &[&str],
        samples: usize,
        options: Options,
        shell: &mut Shell<'_>,
    ) -> io::Result<Sampling> {
        let mut sampling = Sampling::new();
        for _ in 0..samples {
            let collected = self.run_tests(program, names.iter().copied(), options)?.collect(&mut |_| {})?;
            if !sampling.absorb(&collected, Measured::per_iteration, shell) {
                break;
            }
        }
        Ok(sampling)
    }

    /// `samples` runs of each benchmark in a process of its own under `perf`; each benchmark's
    /// samples are the instructions the process retired per iteration of its loop, its start
    /// and its other iterations of the loop's batches counted in.
    ///
    /// # Errors
    /// Fails when the program cannot be started or its output read; `Err` inside says why
    /// instructions are not counted.
    pub(super) fn sample_instructions(
        &self,
        program: &Path,
        names: &[&str],
        samples: usize,
        options: Options,
        shell: &mut Shell<'_>,
    ) -> io::Result<Result<Sampling, Unavailable>> {
        if let Err(unavailable) = Counter::allowed(!self.runner().is_empty()) {
            return Ok(Err(unavailable));
        }
        let listed = self.list_tests(program, names.iter().copied(), options)?;
        let file = Scratch::create("counts")?;
        let into = file.join("counts");
        let perf = Counter { into: &into };
        let mut sampling = Sampling::new();
        for name in &listed {
            for _ in 0..samples {
                let command = self.tests_command(program, vec![OsString::from(name)], options, false);
                let Counted { collected, instructions } = match perf.run(program, &command)? {
                    Ok(counted) => counted,
                    Err(unavailable) => return Ok(Err(unavailable)),
                };
                #[expect(clippy::cast_precision_loss, reason = "a count, averaged")]
                let total = instructions as f64;
                let per = |measured: Measured| measured.per_iteration_of(total);
                if !sampling.absorb(&collected, per, shell) {
                    return Ok(Ok(sampling));
                }
            }
        }
        Ok(Ok(sampling))
    }
}
