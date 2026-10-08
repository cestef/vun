//! The session as a command line runs it.

use std::time::Duration;
use vun_command::{Diagnostics, Outcome};

use vun_term::{Shell, Text, Verb};
use vun_trace::Live;

use crate::Session;

/// The verb of the line saying which stages run.
const WORKING: &str = "Working";
/// How long a command runs before it says which stages it is in.
const STILL: Duration = Duration::from_millis(700);

/// A session run by a command line: what a command refuses is reported, and once it ran the
/// diagnostics are shown, the queries saved, and the timings with how work spread over threads
/// and what the queries did with their memos reported when asked.
impl vun_command::Context for Session {
    fn refused(&mut self, diagnostic: vun_diag::Diagnostic) {
        self.diag.error(diagnostic);
    }

    /// A command that runs long says which stages it is in, once they take a moment.
    fn started(&mut self, shell: &mut Shell<'_>) {
        self.progress = shell.progress();
        if self.progress.is_drawn() {
            Live::watch();
            let stages = || Text::join(Live::stages().into_iter().map(Text::from), ", ");
            self.working = Some(shell.watch(Verb::good(WORKING), STILL, Box::new(stages)));
        }
    }

    fn finish(&mut self, shell: &mut Shell<'_>) {
        self.foreground();
        self.emit(shell);
        self.save();
        if let Some(report) = vun_trace::Timings::report() {
            shell.status(&report);
            shell.status(&vun_par::Usage::now());
        }
        match vun_trace::Timeline::write() {
            Some(Ok(timeline)) => shell.status(&timeline),
            Some(Err(error)) => {
                let path =
                    vun_trace::Timeline::path().expect("a timeline is written where one was asked for");
                self.diag.error(vun_diag::Diagnostic::unwritable(path, &error));
                self.emit(shell);
            }
            None => {}
        }
        if self.tally {
            shell.status(&self.db.tally());
        }
        shell.finish_diagnostics(self.registry.renderer(self.format));
    }
}

impl Session {
    /// Takes the bars off the terminal for good: a program the command starts, or the end of the
    /// command, writes there itself.
    pub fn foreground(&mut self) {
        self.working = None;
        self.progress.clear();
    }

    /// How a command that reported its failures as diagnostics ends.
    #[must_use]
    pub fn outcome(&self) -> Outcome {
        if self.failed() { Outcome::Failure } else { Outcome::Success }
    }
}
