//! Running the tests in parallel, as nextest does: the program lists its tests, then each runs in
//! a process of its own, `jobs` at a time, and its result is reported as it ends. A test that
//! crashes its process takes no other down, and one that never ends holds one worker.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::sync::mpsc;
use std::time::Instant;

use vun_term::{Document, Shell};

use super::report::Collected;
use super::tally::Tally;
use super::{Line, Options, Reported, Totals};
use crate::Session;

impl Session {
    /// The names of the tests `program` would run for the tests `names` and `options`, each once,
    /// in the order they are declared.
    ///
    /// # Errors
    /// Fails when the program cannot be started or its output read.
    pub fn list_tests<S: AsRef<std::ffi::OsStr>>(
        &self,
        program: &Path,
        names: impl IntoIterator<Item = S>,
        options: Options,
    ) -> io::Result<Vec<String>> {
        let names = names.into_iter().map(|n| n.as_ref().to_os_string()).collect();
        let run = self.start_tests(program, names, options, true)?;
        let mut listed = Vec::new();
        let mut seen = BTreeSet::new();
        for text in run.lines()? {
            if let Some(Line::Listed(test)) = Line::of(&text?)
                && seen.insert(test.name.clone())
            {
                listed.push(test.name);
            }
        }
        Ok(listed)
    }

    /// Runs the tests `names` of `program`, each in a process of its own, `jobs` at a time, and
    /// reports each as its process ends, then the totals.
    ///
    /// # Errors
    /// Fails when a program cannot be started or its output read or reported.
    pub fn run_tests_parallel(
        &self,
        program: &Path,
        names: &[String],
        options: Options,
        jobs: usize,
        shell: &mut Shell<'_>,
    ) -> io::Result<Reported> {
        let started = Instant::now();
        let (sender, receiver) = mpsc::channel::<(String, io::Result<Collected>)>();
        let mut queue = names.iter();
        let (mut running, mut passed) = (0_usize, true);
        let (mut totals, mut tests) = (Totals::default(), Vec::new());
        loop {
            while running < jobs.max(1) {
                let Some(name) = queue.next() else { break };
                let run = self.run_tests(program, [OsString::from(name)], options)?;
                let (sender, name) = (sender.clone(), name.clone());
                std::thread::spawn(move || {
                    let _ = sender.send((name, run.collect(&mut |_| {})));
                });
                running += 1;
            }
            if running == 0 {
                break;
            }
            let Ok((name, collected)) = receiver.recv() else { break };
            running -= 1;
            let collected = collected?;
            for tested in &collected.tests {
                shell.status(tested);
            }
            if !collected.output.is_empty() {
                let text = collected.output.join("\n");
                shell.result(&Document { kind: "output", name: name.clone(), text })?;
            }
            // A process that wrote no result of its test crashed, or ended before it got there.
            if collected.tests.is_empty() {
                let text = format!("`{name}` ended its process with {} before it reported", collected.status);
                shell.result(&Document { kind: "output", name, text })?;
                passed = false;
            }
            if let Some(ended) = collected.totals {
                totals.add(&ended);
            }
            passed &= collected.clean();
            tests.extend(collected.tests);
        }
        totals.nanos = u64::try_from(started.elapsed().as_nanos()).ok();
        shell.status(&Tally(totals));
        Ok(Reported { tests, passed, covered: None })
    }
}

impl Totals {
    /// These totals and `other`'s together, the time apart.
    fn add(&mut self, other: &Self) {
        self.passed += other.passed;
        self.trapped += other.trapped;
        self.falsified += other.falsified;
        self.ignored += other.ignored;
        self.measured += other.measured;
        self.fuzzed += other.fuzzed;
    }
}
