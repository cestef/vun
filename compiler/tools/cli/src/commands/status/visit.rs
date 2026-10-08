//! Going through the files of a command one at a time: a bar counting them while the terminal
//! shows bars, and with `-v` a status line for each, as it is begun.

use vun_data::Object;
use vun_term::{Bar, Noun, Record, Shell, Status, Style, Text, Unit, Verb, Verbosity};

use super::Step;

const FILES: Noun = Noun { one: "file", many: "files" };

/// A file a step is begun on, among others: shown with `-v` only, since the step is announced
/// once as a whole.
struct Visiting {
    step: Step,
    subject: String,
}

impl Record for Visiting {
    fn kind(&self) -> &'static str {
        "visiting"
    }

    fn text(&self) -> Text {
        Text::of(Style::Strong, self.subject.clone())
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("step", self.step.name()).with("subject", self.subject.clone())
    }
}

impl Status for Visiting {
    fn verb(&self) -> Verb {
        self.step.verb()
    }

    fn level(&self) -> Verbosity {
        Verbosity::Verbose
    }
}

/// The files of a step, counted as each is begun.
pub(crate) struct Visit {
    step: Step,
    bar: Bar,
}

impl Visit {
    /// A pass of `step` over `total` files.
    pub(crate) fn of(shell: &Shell<'_>, step: Step, total: usize) -> Self {
        let bar =
            shell.bar(step.verb(), Text::new(), u64::try_from(total).unwrap_or(u64::MAX), Unit::Items(FILES));
        Self { step, bar }
    }

    /// The previous file is done, and the one `subject` is begun.
    pub(crate) fn begin(&self, shell: &mut Shell<'_>, subject: String) {
        self.bar.subject(Text::of(Style::Strong, subject.clone()));
        shell.status(&Visiting { step: self.step, subject });
    }

    /// The file begun is done.
    pub(crate) fn end(&self) {
        self.bar.advance(1);
    }
}
