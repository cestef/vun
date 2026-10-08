//! Which tests a command asks for, and what is said when none is run: nothing declared, or
//! names and a filter matching nothing.

use std::io;
use std::path::Path;

use vun_data::Object;
use vun_diag::{Diagnostic, Msg, errors};
use vun_term::{Noun, Record, Shell, Status, Style, Text, Verb};

use super::{FILTER, Mode, Options};
use crate::Session;

/// The tests a command line names: by name, and by a text their names hold.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection<'a> {
    pub names: Vec<&'a str>,
    pub filter: Option<&'a str>,
}

impl Selection<'_> {
    /// Whether it asks for every test.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty() && self.filter.is_none()
    }

    /// What the tests' program is given to choose the tests: the names, then the filter.
    #[must_use]
    pub fn arguments(&self) -> Vec<String> {
        let filter = self.filter.into_iter().flat_map(|text| [FILTER.long_form(), text.to_owned()]);
        self.names.iter().map(|&name| name.to_owned()).chain(filter).collect()
    }
}

impl Mode {
    /// What the run chooses among: tests, benchmarks or properties.
    #[must_use]
    pub const fn noun(self) -> Noun {
        match self {
            Self::Test | Self::Cover => Noun { one: "test", many: "tests" },
            Self::Bench => Noun { one: "benchmark", many: "benchmarks" },
            Self::Fuzz => Noun { one: "property", many: "properties" },
        }
    }
}

/// A run with nothing to run, since the namespace declares none.
struct Nothing(Noun);

impl Record for Nothing {
    fn kind(&self) -> &'static str {
        "nothing_declared"
    }

    fn text(&self) -> Text {
        Text::of(Style::Dim, format!("the namespace declares no {}", self.0.many))
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("noun", self.0.many)
    }
}

impl Status for Nothing {
    fn verb(&self) -> Verb {
        Verb::warned("Skipped")
    }
}

impl Session {
    /// The names of the tests `program` declares for `options`, each once.
    ///
    /// # Errors
    /// Fails when the program cannot be started or its output read.
    pub fn listed(
        &self,
        program: &Path,
        selection: &Selection<'_>,
        options: Options,
    ) -> io::Result<Vec<String>> {
        self.list_tests(program, selection.arguments(), options)
    }

    /// Reports the names and the filter of `selection` that match no test `program` declares,
    /// the closest name suggested; `listed` is what it selects. Whether any was reported.
    ///
    /// # Errors
    /// Fails when the program cannot be started or its output read.
    pub fn unmatched(
        &mut self,
        program: &Path,
        selection: &Selection<'_>,
        options: Options,
        listed: &[String],
        shell: &mut Shell<'_>,
    ) -> io::Result<bool> {
        let missing = |name: &&str| !listed.iter().any(|l| l == name);
        if selection.is_empty() || (!listed.is_empty() && !selection.names.iter().any(missing)) {
            return Ok(false);
        }
        let noun = options.mode.noun();
        let declared = self.listed(program, &Selection::default(), options)?;
        let known = || declared.iter().map(String::as_str);
        let overview = || {
            if declared.is_empty() {
                Msg::new().text("this namespace declares no ").text(noun.many)
            } else {
                Msg::new().text("this namespace declares ").code_list(known())
            }
        };
        let before = self.diag.error_count();
        for name in selection.names.iter().filter(|name| !known().any(|declared| declared == **name)) {
            let msg = Msg::new().text("no ").text(noun.one).text(" is named ").code(*name);
            let help = vun_suggest::closest(name, known()).map_or_else(overview, Msg::did_you_mean);
            self.diag.error(Diagnostic::error(errors::TEST_UNMATCHED, msg).help(help));
        }
        if let (Some(text), true) = (selection.filter, listed.is_empty() && self.diag.error_count() == before)
        {
            let msg = Msg::new().text("no ").text(noun.one).text(" has a name containing ").code(text);
            self.diag.error(Diagnostic::error(errors::TEST_UNMATCHED, msg).help(overview()));
        }
        self.emit(shell);
        Ok(true)
    }

    /// Says that the namespace declares no tests, benchmarks or properties.
    pub fn nothing_declared(&self, options: Options, shell: &mut Shell<'_>) {
        shell.status(&Nothing(options.mode.noun()));
    }
}
