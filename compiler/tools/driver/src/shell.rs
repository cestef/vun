//! The session as a command line runs it.

use vun_command::Outcome;
use vun_term::Shell;

use crate::Session;

/// A session run by a command line: what a command refuses is reported, and once it ran the
/// diagnostics are shown, the queries saved, and the timings with how work spread over threads
/// and what the queries did with their memos reported when asked.
impl vun_command::Context for Session {
    fn refused(&mut self, diagnostic: vun_diag::Diagnostic) {
        self.diag.error(diagnostic);
    }

    fn finish(&mut self, shell: &mut Shell<'_>) {
        self.emit(shell);
        self.save();
        if let Some(report) = vun_trace::Timings::report() {
            shell.status(&report);
            shell.status(&vun_par::Usage::now());
        }
        if self.tally {
            shell.status(&self.db.tally());
        }
    }
}

impl Session {
    /// How a command that reported its failures as diagnostics ends.
    #[must_use]
    pub fn outcome(&self) -> Outcome {
        if self.failed() { Outcome::Failure } else { Outcome::Success }
    }
}
