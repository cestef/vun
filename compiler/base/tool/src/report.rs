//! How the failure of a tool is reported.

use std::io::ErrorKind;
use std::path::Path;

use vun_diag::{Diagnostic, Msg, errors};

use crate::Failure;

/// What a tool was run to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    Compile,
    Link,
    Archive,
}

impl Task {
    fn verb(self) -> &'static str {
        match self {
            Self::Compile => " could not compile ",
            Self::Link => " could not link ",
            Self::Archive => " could not archive ",
        }
    }
}

impl Failure {
    /// The program is not installed, as opposed to having failed.
    #[must_use]
    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Start(e) if e.kind() == ErrorKind::NotFound)
    }

    /// `program` failing at `task` on `subject`: a program that is not installed says so, and
    /// where to see what the target's build needs.
    #[must_use]
    pub fn report(&self, program: &str, task: Task, subject: &Path) -> Diagnostic {
        let msg = Msg::new().code(program).text(task.verb()).code(subject.display().to_string());
        let d = Diagnostic::error(errors::BACKEND_TOOL, msg).caused_by(self);
        if !self.is_missing() {
            return d;
        }
        let note = Msg::new().code(program).text(" is not on the search path");
        let help = Msg::new()
            .text("install it, or name another with the machine's tools file; ")
            .code("vun target list")
            .text(" shows what each target's build needs");
        d.note(note).help(help)
    }
}
