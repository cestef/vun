//! What commands report as they go, as cargo does: a status per step, and a status ending each
//! command that compiles, after its diagnostics. Each is a record, shown as a status line for
//! people and as a JSON line for programs.

mod hash;
mod packages;

use std::time::{Duration, Instant};

use vun_diag::Severity;
use vun_json::Object;
use vun_term::{Elapsed, Noun, Record, Shell, Status, Style, Text, Verb};

pub(super) use hash::{HashText, Shown};
pub(super) use packages::{Added, Collected, Packed, Source, UpToDate, Updated};

use crate::Session;

const ERROR: Noun = Noun { one: "error", many: "errors" };
const WARNING: Noun = Noun { one: "warning", many: "warnings" };

/// A step of a compiling command.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Step {
    Check,
    Build,
    Run,
}

impl Step {
    const fn name(self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::Build => "build",
            Self::Run => "run",
        }
    }
}

/// A step begun on a file or a program.
pub(super) struct Started {
    pub(super) step: Step,
    pub(super) subject: String,
}

impl Record for Started {
    fn kind(&self) -> &'static str {
        "started"
    }

    fn text(&self) -> Text {
        Text::of(Style::Strong, self.subject.clone())
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("step", self.step.name()).with("subject", self.subject.clone())
    }
}

impl Status for Started {
    fn verb(&self) -> Verb {
        match self.step {
            Step::Check => Verb::good("Checking"),
            Step::Build => Verb::good("Building"),
            Step::Run => Verb::good("Running"),
        }
    }
}

/// How a compiling command ended.
pub(super) struct Finished {
    subject: String,
    elapsed: Duration,
    errors: usize,
    warnings: usize,
}

impl Record for Finished {
    fn kind(&self) -> &'static str {
        "finished"
    }

    fn text(&self) -> Text {
        let text = Text::of(Style::Strong, self.subject.clone());
        let text = if self.errors > 0 {
            text.plain(" with ").styled(Style::Error, ERROR.count(self.errors).to_string())
        } else {
            text.plain(" in ").plain(Elapsed(self.elapsed).to_string())
        };
        let warned = Text::new().plain(", ").styled(Style::Warning, WARNING.count(self.warnings).to_string());
        if self.warnings > 0 { text.append(warned) } else { text }
    }

    fn fields(&self) -> Object<'static> {
        let millis = u64::try_from(self.elapsed.as_millis()).unwrap_or(u64::MAX);
        Object::new()
            .with("subject", self.subject.clone())
            .with("success", self.errors == 0)
            .with("milliseconds", millis)
            .with("errors", self.errors)
            .with("warnings", self.warnings)
    }
}

impl Status for Finished {
    fn verb(&self) -> Verb {
        if self.errors > 0 { Verb::failed("Failed") } else { Verb::good("Finished") }
    }
}

impl Session {
    /// Shows the diagnostics collected, then how the step on `subject` started at `started`
    /// ended.
    pub(super) fn conclude(&mut self, shell: &mut Shell<'_>, subject: String, started: Instant) {
        let (errors, warnings) = (self.diag.count(Severity::Error), self.diag.count(Severity::Warning));
        // Errors shown earlier, such as while fetching, fail the step as well.
        let errors = if self.failed() { errors.max(1) } else { errors };
        self.emit(shell);
        shell.status(&Finished { subject, elapsed: started.elapsed(), errors, warnings });
    }
}
