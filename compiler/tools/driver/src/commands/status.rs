//! What commands report as they go, as cargo does: a status line per step, and a line ending
//! each command that compiles, after its diagnostics.

use std::time::Instant;

use vun_diag::Severity;
use vun_hash::Hash;
use vun_intern::Symbol;
use vun_term::{Elapsed, Noun, Shell, Style, Text, Verb};

use crate::Session;
use crate::packages::Fetched;

pub(super) const CHECKING: Verb = Verb::good("Checking");
pub(super) const BUILDING: Verb = Verb::good("Building");
pub(super) const RUNNING: Verb = Verb::good("Running");
pub(super) const FINISHED: Verb = Verb::good("Finished");
pub(super) const FAILED: Verb = Verb::failed("Failed");
pub(super) const DOWNLOADED: Verb = Verb::good("Downloaded");
pub(super) const ADDED: Verb = Verb::good("Added");
pub(super) const UPDATED: Verb = Verb::good("Updated");
/// Nothing needed doing.
pub(super) const FRESH: Verb = Verb::good("Fresh");
pub(super) const PACKED: Verb = Verb::good("Packed");
pub(super) const REMOVED: Verb = Verb::good("Removed");

const ERROR: Noun = Noun { one: "error", many: "errors" };
const WARNING: Noun = Noun { one: "warning", many: "warnings" };

impl Session {
    /// Shows the diagnostics collected, then how the step started at `started` on `subject`
    /// ended: `Finished` with its time and warnings, or `Failed` with its errors.
    pub(super) fn conclude(&mut self, shell: &mut Shell<'_>, subject: Text, started: Instant) {
        let (errors, warnings) = (self.diag.count(Severity::Error), self.diag.count(Severity::Warning));
        let failed = self.failed();
        self.emit(shell);
        let warned = (warnings > 0)
            .then(|| Text::new().plain(", ").styled(Style::Warning, WARNING.count(warnings).to_string()));
        if failed {
            let errors = Text::new().plain(" with ").styled(Style::Error, ERROR.count(errors).to_string());
            shell.status(FAILED, subject.append(errors).append(warned.unwrap_or_default()));
        } else {
            let time = Text::new().plain(" in ").plain(Elapsed(started.elapsed()).to_string());
            shell.status(FINISHED, subject.append(time).append(warned.unwrap_or_default()));
        }
    }
}

impl Fetched {
    /// Reports this outcome of fetching `name` at `hash`, when a download happened.
    pub(super) fn report(&self, name: Symbol, hash: Hash, shell: &mut Shell<'_>) {
        if let Self::Stored { url } = self {
            let line =
                Text::of(Style::Strong, name.as_str().to_owned()).plain(" ").append(hash.text(Shown::Short));
            shell.status(DOWNLOADED, line.plain(" from ").plain(url.clone()));
        }
    }
}

/// How much of a content hash a line shows.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Shown {
    /// Enough to tell stored packages apart, as jj shows change ids.
    Short,
    Full,
}

/// The digest characters of a short hash.
const SHORT: usize = 12;

/// Content hashes as output shows them.
pub(super) trait HashText {
    /// Its algorithm dimmed, its digest as an id.
    fn text(&self, shown: Shown) -> Text;
}

impl HashText for Hash {
    fn text(&self, shown: Shown) -> Text {
        let digest = self.digest();
        let digest = if shown == Shown::Short { digest.chars().take(SHORT).collect() } else { digest };
        let algorithm = format!("{}{}", self.algorithm().name(), Self::SEPARATOR);
        Text::of(Style::Dim, algorithm).styled(Style::Id, digest)
    }
}
